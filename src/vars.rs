use std::env::var;
use std::net::IpAddr;
use url::Url;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

lazy_static! {
    pub static ref SERVER_BANNER: String = format!("{} {}", NAME, VERSION);
}

// Regskin config env vars.
lazy_static! {
    pub static ref REGSKIN_REGISTRY_URL: String = var("REGSKIN_REGISTRY_URL").unwrap();
    pub static ref REGSKIN_REGISTRY_HOST: String =
        Url::parse(&var("REGSKIN_REGISTRY_URL").unwrap())
            .unwrap()
            .host_str()
            .unwrap()
            .to_string();
    pub static ref REGSKIN_LISTEN: IpAddr = var("REGSKIN_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .unwrap();
    pub static ref REGSKIN_PORT: u16 = var("REGSKIN_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap();
}
