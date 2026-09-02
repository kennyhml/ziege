use std::{env, error::Error, fs, io};

use zadt::ReqwestTransport;

pub fn reqwest_transport() -> Result<ReqwestTransport, Box<dyn Error>> {
    let mut builder = ReqwestTransport::builder()
        .destination(required_env("SAP_DESTINATION")?)
        .sap_client(required_env("SAP_CLIENT")?)
        .language(env::var("SAP_LANGUAGE").unwrap_or_else(|_| "EN".to_owned()))
        .basic_auth(required_env("SAP_USERNAME")?, required_env("SAP_PASSWORD")?)
        .danger_accept_invalid_certs(env_flag("SAP_DANGER_ACCEPT_INVALID_CERTS"))
        .danger_accept_invalid_hostnames(env_flag("SAP_DANGER_ACCEPT_INVALID_HOSTNAMES"));

    if let Some(path) = env::var_os("SAP_TLS_ROOT_CERTIFICATE") {
        builder = builder.add_root_certificate_pem(fs::read(path)?);
    }

    Ok(builder.build()?)
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn required_env(name: &str) -> Result<String, io::Error> {
    env::var(name).map_err(|source| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing required environment variable `{name}`: {source}"),
        )
    })
}
