use clap::Args;

/// Executa o ciclo de descoberta autônoma.
#[derive(Args, Debug, Clone)]
pub struct DiscoverArgs {
    /// Termo de busca a ser utilizado nas engines suportadas
    #[arg(short, long)]
    pub query: String,

    /// Número máximo de PLANs a serem criados durante a execução
    #[arg(short = 'm', long, default_value_t = 10)]
    pub max_plans: usize,

    /// Define a search engine (google | bing | duckduckgo)
    #[arg(long, value_parser = ["google", "bing", "duckduckgo", "ddg"], value_name = "ENGINE")]
    pub search_engine: Option<String>,

    /// Executa sem criar PLANs, apenas relatando os candidatos encontrados
    #[arg(long)]
    pub dry_run: bool,

    /// Habilita logs detalhados da execução do discovery loop
    #[arg(long)]
    pub debug: bool,
}
