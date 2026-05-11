//! `nt publish` — stub for TDD RED. Implementation lands in the GREEN
//! commit; for now the body panics so wiremock tests fail clearly.

pub struct PublishArgs<'a> {
    pub type_id: &'a str,
    pub data: &'a serde_json::Value,
    pub project: &'a str,
    pub profile: Option<&'a str>,
}

pub async fn run(_args: PublishArgs<'_>) -> i32 {
    panic!("nt-cli::commands::publish::run not yet implemented");
}
