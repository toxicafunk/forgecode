use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use forge_app::domain::{
    McpConfig, McpServerConfig, McpServers, ServerName, ToolCallFull, ToolDefinition, ToolName,
    ToolOutput,
};
use forge_app::{
    EnvironmentInfra, KVStore, McpClientInfra, McpConfigManager, McpServerInfra, McpService,
};
use tokio::sync::{Mutex, RwLock};

use crate::mcp::tool::McpExecutor;

#[derive(Clone)]
pub struct ForgeMcpService<M, I, C> {
    tools: Arc<RwLock<HashMap<ToolName, ToolHolder<McpExecutor<C>>>>>,
    failed_servers: Arc<RwLock<HashMap<ServerName, String>>>,
    previous_config_hash: Arc<Mutex<u64>>,
    manager: Arc<M>,
    infra: Arc<I>,
}

#[derive(Clone)]
struct ToolHolder<T> {
    definition: ToolDefinition,
    executable: T,
    server_name: String,
}

impl<M, I, C> ForgeMcpService<M, I, C>
where
    M: McpConfigManager,
    I: McpServerInfra + KVStore + EnvironmentInfra,
    C: McpClientInfra + Clone,
    C: From<<I as McpServerInfra>::Client>,
{
    pub fn new(manager: Arc<M>, infra: Arc<I>) -> Self {
        Self {
            tools: Default::default(),
            failed_servers: Default::default(),
            previous_config_hash: Arc::new(Mutex::new(Default::default())),
            manager,
            infra,
        }
    }

    async fn is_config_modified(&self, config: &McpConfig) -> bool {
        *self.previous_config_hash.lock().await != config.cache_key()
    }

    async fn insert_clients(&self, server_name: &ServerName, client: Arc<C>) -> anyhow::Result<()> {
        let tools = client.list().await?;

        let mut tool_map = self.tools.write().await;

        for mut tool in tools.into_iter() {
            let actual_name = tool.name.clone();
            let server = McpExecutor::new(actual_name, client.clone())?;

            // Generate a unique name for the tool
            let generated_name = ToolName::new(format!(
                "mcp_{server_name}_tool_{}",
                tool.name.into_sanitized()
            ));

            tool.name = generated_name.clone();

            tool_map.insert(
                generated_name,
                ToolHolder {
                    definition: tool,
                    executable: server,
                    server_name: server_name.to_string(),
                },
            );
        }

        Ok(())
    }

    async fn connect(
        &self,
        server_name: &ServerName,
        config: McpServerConfig,
    ) -> anyhow::Result<()> {
        let env_vars = self.infra.get_env_vars();
        let client = self.infra.connect(config, &env_vars).await?;
        let client = Arc::new(C::from(client));
        self.insert_clients(server_name, client).await?;

        Ok(())
    }

    async fn init_mcp(&self) -> anyhow::Result<()> {
        let mcp = self.manager.read_mcp_config(None).await?;

        // If config is unchanged, skip reinitialization
        if !self.is_config_modified(&mcp).await {
            return Ok(());
        }

        self.update_mcp(mcp).await
    }

    async fn update_mcp(&self, mcp: McpConfig) -> Result<(), anyhow::Error> {
        // Update the hash with the new config
        let new_hash = mcp.cache_key();
        *self.previous_config_hash.lock().await = new_hash;
        self.clear_tools().await;

        // Clear failed servers map before attempting new connections
        self.failed_servers.write().await.clear();

        let connections: Vec<_> = mcp
            .mcp_servers
            .into_iter()
            .filter(|v| !v.1.is_disabled())
            .map(|(name, server)| async move {
                let conn = self
                    .connect(&name, server)
                    .await
                    .context(format!("Failed to initiate MCP server: {name}"));

                (name, conn)
            })
            .collect();

        let results = futures::future::join_all(connections).await;

        for (server_name, result) in results {
            match result {
                Ok(_) => {}
                Err(error) => {
                    // Format error with full chain for detailed diagnostics
                    // Using Debug formatting with alternate flag shows the full error chain
                    let error_string = format!("{error:?}");
                    self.failed_servers
                        .write()
                        .await
                        .insert(server_name.clone(), error_string.clone());
                }
            }
        }

        Ok(())
    }

    async fn list(&self) -> anyhow::Result<McpServers> {
        self.init_mcp().await?;

        let tools = self.tools.read().await;
        let mut grouped_tools = std::collections::HashMap::new();

        for tool in tools.values() {
            grouped_tools
                .entry(ServerName::from(tool.server_name.clone()))
                .or_insert_with(Vec::new)
                .push(tool.definition.clone());
        }

        let failures = self.failed_servers.read().await.clone();

        Ok(McpServers::new(grouped_tools, failures))
    }
    async fn clear_tools(&self) {
        self.tools.write().await.clear()
    }

    async fn call(&self, call: ToolCallFull) -> anyhow::Result<ToolOutput> {
        // Ensure MCP connections are initialized before calling tools
        self.init_mcp().await?;

        let tools = self.tools.read().await;

        let tool = tools.get(&call.name).context("Tool not found")?;

        tool.executable.call_tool(call.arguments.parse()?).await
    }

    /// Refresh the MCP cache by fetching fresh data
    async fn refresh_cache(&self) -> anyhow::Result<()> {
        // Reset the in-memory config hash so that init_mcp() treats the next
        // call as a config change and forces a full reconnect to all MCP
        // servers, even when the config on disk hasn't changed.
        *self.previous_config_hash.lock().await = 0;
        self.infra.cache_clear().await?;
        let _ = self.get_mcp_servers().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::hash::Hash;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use forge_app::domain::{
        McpConfig, McpServerConfig, Scope, ServerName, ToolDefinition, ToolName,
        ToolOutput,
    };
    use forge_app::{EnvironmentInfra, KVStore, McpClientInfra, McpConfigManager, McpServerInfra};
    use forge_domain::Environment;
    use pretty_assertions::assert_eq;

    use super::*;

    // ---------------------------------------------------------------------------
    // Minimal mock: MCP client
    // ---------------------------------------------------------------------------

    #[derive(Clone)]
    struct MockClient;

    #[async_trait]
    impl McpClientInfra for MockClient {
        async fn list(&self) -> anyhow::Result<Vec<ToolDefinition>> {
            Ok(vec![ToolDefinition::new("mock_tool").description("A mock tool")])
        }

        async fn call(
            &self,
            _tool_name: &ToolName,
            _input: serde_json::Value,
        ) -> anyhow::Result<ToolOutput> {
            Ok(ToolOutput::text("mock result"))
        }
    }



    // ---------------------------------------------------------------------------
    // Minimal mock: infrastructure (McpServerInfra + KVStore + EnvironmentInfra)
    // ---------------------------------------------------------------------------

    #[derive(Clone)]
    struct MockInfra {
        connect_count: Arc<AtomicUsize>,
    }

    impl MockInfra {
        fn new() -> (Arc<Self>, Arc<AtomicUsize>) {
            let connect_count = Arc::new(AtomicUsize::new(0));
            let infra = Arc::new(Self { connect_count: connect_count.clone() });
            (infra, connect_count)
        }
    }

    #[async_trait]
    impl McpServerInfra for MockInfra {
        type Client = MockClient;

        async fn connect(
            &self,
            _config: McpServerConfig,
            _env_vars: &BTreeMap<String, String>,
        ) -> anyhow::Result<Self::Client> {
            self.connect_count.fetch_add(1, Ordering::SeqCst);
            Ok(MockClient)
        }
    }

    #[async_trait]
    impl KVStore for MockInfra {
        async fn cache_get<K, V>(&self, _key: &K) -> anyhow::Result<Option<V>>
        where
            K: Hash + Sync,
            V: serde::Serialize + serde::de::DeserializeOwned + Send,
        {
            // Always miss so every get_mcp_servers() call exercises init_mcp
            Ok(None)
        }

        async fn cache_set<K, V>(&self, _key: &K, _value: &V) -> anyhow::Result<()>
        where
            K: Hash + Sync,
            V: serde::Serialize + Sync,
        {
            Ok(())
        }

        async fn cache_clear(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    impl EnvironmentInfra for MockInfra {
        fn get_environment(&self) -> Environment {
            use fake::{Fake, Faker};
            Faker.fake()
        }

        fn get_env_var(&self, _key: &str) -> Option<String> {
            None
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        fn is_restricted(&self) -> bool {
            false
        }
    }

    // ---------------------------------------------------------------------------
    // Minimal mock: MCP config manager
    // ---------------------------------------------------------------------------

    struct MockManager {
        config: McpConfig,
    }

    impl MockManager {
        fn with_one_server() -> Self {
            let config = McpConfig::from(BTreeMap::from([(
                ServerName::from("test-server".to_string()),
                McpServerConfig::new_http("http://localhost:9999"),
            )]));
            Self { config }
        }
    }

    #[async_trait]
    impl McpConfigManager for MockManager {
        async fn read_mcp_config(
            &self,
            _scope: Option<&Scope>,
        ) -> anyhow::Result<McpConfig> {
            Ok(self.config.clone())
        }

        async fn write_mcp_config(
            &self,
            _config: &McpConfig,
            _scope: &Scope,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    // ---------------------------------------------------------------------------
    // Helper
    // ---------------------------------------------------------------------------

    fn fixture() -> (ForgeMcpService<MockManager, MockInfra, MockClient>, Arc<AtomicUsize>) {
        let (infra, connect_count) = MockInfra::new();
        let manager = Arc::new(MockManager::with_one_server());
        let service = ForgeMcpService::new(manager, infra);
        (service, connect_count)
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    /// The first call to get_mcp_servers() must connect to every configured
    /// server.  A second call with an unchanged config must reuse the
    /// in-memory tool map and must NOT reconnect.
    #[tokio::test]
    async fn test_second_call_does_not_reconnect() {
        let (service, connect_count) = fixture();

        service.get_mcp_servers().await.unwrap();
        let actual_after_first = connect_count.load(Ordering::SeqCst);
        assert_eq!(actual_after_first, 1);

        service.get_mcp_servers().await.unwrap();
        let actual_after_second = connect_count.load(Ordering::SeqCst);
        assert_eq!(actual_after_second, 1);
    }

    /// After reload_mcp() the previous_config_hash must be reset to 0 so that
    /// the next get_mcp_servers() call triggers a full reconnect even though
    /// the config on disk has not changed.
    #[tokio::test]
    async fn test_reload_mcp_forces_reconnect() {
        let (service, connect_count) = fixture();

        // Establish initial connection
        service.get_mcp_servers().await.unwrap();
        assert_eq!(connect_count.load(Ordering::SeqCst), 1);

        // Reload should reset the hash; next call must reconnect
        service.reload_mcp().await.unwrap();
        service.get_mcp_servers().await.unwrap();
        let actual = connect_count.load(Ordering::SeqCst);
        let expected = 2;
        assert_eq!(actual, expected);
    }

    /// Tools returned by get_mcp_servers() must be prefixed with the
    /// server name so callers can identify their provenance.
    #[tokio::test]
    async fn test_tool_names_are_prefixed_with_server_name() {
        let (service, _) = fixture();

        let servers = service.get_mcp_servers().await.unwrap();
        let tool_names: Vec<String> = servers
            .get_servers()
            .values()
            .flat_map(|tools| tools.iter().map(|t| t.name.to_string()))
            .collect();

        assert!(
            tool_names
                .iter()
                .all(|name| name.starts_with("mcp_test-server_tool_")),
            "every MCP tool name must carry the server prefix; got: {tool_names:?}"
        );
    }
}

#[async_trait::async_trait]
impl<M: McpConfigManager, I: McpServerInfra + KVStore + EnvironmentInfra, C> McpService
    for ForgeMcpService<M, I, C>
where
    C: McpClientInfra + Clone,
    C: From<<I as McpServerInfra>::Client>,
{
    async fn get_mcp_servers(&self) -> anyhow::Result<McpServers> {
        // Read current configs to compute merged hash
        let mcp_config = self.manager.read_mcp_config(None).await?;

        // Compute unified hash from merged config
        let config_hash = mcp_config.cache_key();

        // Check if cache is valid (exists and not expired)
        // Cache is valid, retrieve it
        if let Some(cache) = self.infra.cache_get::<_, McpServers>(&config_hash).await? {
            return Ok(cache.clone());
        }

        let servers = self.list().await?;
        self.infra.cache_set(&config_hash, &servers).await?;
        Ok(servers)
    }

    async fn execute_mcp(&self, call: ToolCallFull) -> anyhow::Result<ToolOutput> {
        self.call(call).await
    }

    async fn reload_mcp(&self) -> anyhow::Result<()> {
        self.refresh_cache().await
    }
}
