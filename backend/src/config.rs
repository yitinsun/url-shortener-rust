use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Dev,
    Prod,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub env: AppEnv,
    pub base_url: String,
    pub bind_addr: String,
    pub db_host: String,
    pub db_port: u16,
    pub db_name: String,
    pub db_user: String,
    db_password: String,
    pub apply_schema: bool,
    pub openapi_enabled: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self, AppError> {
        let env = match lookup("APP_ENV").as_deref() {
            Some("prod") => AppEnv::Prod,
            Some("test") => AppEnv::Test,
            _ => AppEnv::Dev,
        };
        let dev_like = env != AppEnv::Prod;

        let base_url = required_or(env, lookup("APP_BASE_URL"), "APP_BASE_URL", "http://localhost:8080")?;
        let base_url = base_url.trim_end_matches('/').to_string();
        let bind_addr = nonempty(lookup("BIND_ADDR")).unwrap_or_else(|| "0.0.0.0:8080".to_string());
        let db_host = required_or(env, lookup("DB_HOST"), "DB_HOST", "localhost")?;
        let db_port = nonempty(lookup("DB_PORT"))
            .unwrap_or_else(|| "3306".to_string())
            .parse::<u16>()
            .map_err(|_| AppError::Unavailable("DB_PORT is not a number".to_string()))?;
        let db_name = match env {
            AppEnv::Prod => nonempty(lookup("DB_NAME")).unwrap_or_else(|| "shortener_prod".to_string()),
            AppEnv::Test | AppEnv::Dev => {
                nonempty(lookup("DB_NAME")).unwrap_or_else(|| "shortener_dev".to_string())
            }
        };
        let db_user = required_or(env, lookup("DB_USER"), "DB_USER", "root")?;
        let db_password = match env {
            AppEnv::Prod => nonempty(lookup("DB_PASSWORD")).ok_or_else(|| {
                AppError::Unavailable("DB_PASSWORD is required in prod".to_string())
            })?,
            AppEnv::Dev | AppEnv::Test => {
                nonempty(lookup("DB_PASSWORD")).unwrap_or_else(|| "Tiger".to_string())
            }
        };
        let openapi_enabled = match nonempty(lookup("OPENAPI_ENABLED")) {
            Some(value) => value.eq_ignore_ascii_case("true"),
            None => env != AppEnv::Prod,
        };

        Ok(Self {
            env,
            base_url,
            bind_addr,
            db_host,
            db_port,
            db_name,
            db_user,
            db_password,
            apply_schema: dev_like,
            openapi_enabled,
        })
    }

    pub fn bootstrap_url(&self) -> String {
        mysql_url(
            &self.db_user,
            &self.db_password,
            &self.db_host,
            self.db_port,
            "mysql",
            self.env != AppEnv::Prod,
        )
    }

    pub fn database_url(&self) -> String {
        mysql_url(
            &self.db_user,
            &self.db_password,
            &self.db_host,
            self.db_port,
            &self.db_name,
            self.env != AppEnv::Prod,
        )
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|item| !item.is_empty())
}

fn required_or(
    env: AppEnv,
    value: Option<String>,
    name: &str,
    dev_default: &str,
) -> Result<String, AppError> {
    if let Some(value) = nonempty(value) {
        return Ok(value);
    }
    if env == AppEnv::Prod {
        return Err(AppError::Unavailable(format!("{name} is required in prod")));
    }
    Ok(dev_default.to_string())
}

fn mysql_url(user: &str, password: &str, host: &str, port: u16, database: &str, disable_ssl: bool) -> String {
    let ssl = if disable_ssl {
        "ssl-mode=disabled"
    } else {
        "ssl-mode=required"
    };
    format!(
        "mysql://{user}:{password}@{host}:{port}/{database}?charset=utf8mb4&{ssl}",
        user = encode_userinfo(user),
        password = encode_userinfo(password),
        host = host,
        port = port,
        database = database,
    )
}

fn encode_userinfo(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{AppEnv, Config};

    #[test]
    fn dev_uses_lab_defaults() {
        let config = Config::from_lookup(|_| None).unwrap();
        assert_eq!(config.env, AppEnv::Dev);
        assert_eq!(config.base_url, "http://localhost:8080");
        assert!(config.apply_schema);
        assert!(config.openapi_enabled);
        assert!(config.database_url().contains("shortener_dev"));
        assert!(config.database_url().contains("ssl-mode=disabled"));
    }

    #[test]
    fn prod_rejects_a_missing_password() {
        let error = Config::from_lookup(|name| match name {
            "APP_ENV" => Some("prod".to_string()),
            "APP_BASE_URL" => Some("https://short.example".to_string()),
            "DB_HOST" => Some("db".to_string()),
            "DB_USER" => Some("app".to_string()),
            _ => None,
        });
        assert!(error.is_err());
    }

    #[test]
    fn prod_builds_an_ssl_url_and_encodes_the_password() {
        let config = Config::from_lookup(|name| match name {
            "APP_ENV" => Some("prod".to_string()),
            "APP_BASE_URL" => Some("https://short.example/".to_string()),
            "DB_HOST" => Some("db".to_string()),
            "DB_PORT" => Some("3307".to_string()),
            "DB_USER" => Some("app".to_string()),
            "DB_PASSWORD" => Some("p@ss word".to_string()),
            "BIND_ADDR" => Some("127.0.0.1:8080".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.env, AppEnv::Prod);
        assert!(!config.apply_schema);
        assert!(!config.openapi_enabled);
        assert_eq!(config.base_url, "https://short.example");
        assert_eq!(config.db_name, "shortener_prod");
        assert_eq!(config.db_port, 3307);
        assert_eq!(config.bind_addr, "127.0.0.1:8080");
        let url = config.database_url();
        assert!(url.contains("ssl-mode=required"));
        assert!(url.contains("p%40ss%20word"));
        assert!(url.contains("/shortener_prod?"));
        assert!(config.bootstrap_url().contains("/mysql?"));
        assert!(format!("{config:?}").contains("shortener_prod"));
        assert_eq!(config.clone().db_host, "db");
    }

    #[test]
    fn empty_values_fall_back_and_a_bad_port_is_rejected() {
        let config = Config::from_lookup(|name| match name {
            "APP_ENV" => Some("test".to_string()),
            "APP_BASE_URL" => Some("".to_string()),
            "DB_HOST" => Some("".to_string()),
            "DB_PORT" => Some("".to_string()),
            "DB_NAME" => Some("custom".to_string()),
            "DB_USER" => Some("".to_string()),
            "DB_PASSWORD" => Some("".to_string()),
            "BIND_ADDR" => Some("".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.env, AppEnv::Test);
        assert!(config.apply_schema);
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.db_host, "localhost");
        assert_eq!(config.db_port, 3306);
        assert_eq!(config.db_name, "custom");
        assert_eq!(config.db_user, "root");
        assert_eq!(config.bind_addr, "0.0.0.0:8080");
        assert!(config.database_url().contains("ssl-mode=disabled"));

        let error = Config::from_lookup(|name| match name {
            "DB_PORT" => Some("nope".to_string()),
            _ => None,
        });
        assert!(error.is_err());
    }

    #[test]
    fn prod_requires_base_url_host_and_user() {
        for missing in ["APP_BASE_URL", "DB_HOST", "DB_USER"] {
            let error = Config::from_lookup(|name| {
                if name == "APP_ENV" {
                    return Some("prod".to_string());
                }
                if name == missing {
                    return Some("".to_string());
                }
                match name {
                    "APP_BASE_URL" => Some("https://short.example".to_string()),
                    "DB_HOST" => Some("db".to_string()),
                    "DB_USER" => Some("app".to_string()),
                    "DB_PASSWORD" => Some("secret".to_string()),
                    _ => None,
                }
            });
            assert!(error.is_err(), "{missing}");
        }
    }

    #[test]
    fn unknown_env_uses_dev_defaults() {
        let config = Config::from_lookup(|name| match name {
            "APP_ENV" => Some("staging".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.env, AppEnv::Dev);
        assert!(config.openapi_enabled);
        assert_eq!(config, config.clone());
    }

    #[test]
    fn openapi_flag_overrides_the_env_default() {
        let enabled = Config::from_lookup(|name| match name {
            "APP_ENV" => Some("prod".to_string()),
            "APP_BASE_URL" => Some("https://short.example".to_string()),
            "DB_HOST" => Some("db".to_string()),
            "DB_USER" => Some("app".to_string()),
            "DB_PASSWORD" => Some("secret".to_string()),
            "OPENAPI_ENABLED" => Some("true".to_string()),
            _ => None,
        })
        .unwrap();
        assert!(enabled.openapi_enabled);

        let disabled = Config::from_lookup(|name| match name {
            "OPENAPI_ENABLED" => Some("false".to_string()),
            _ => None,
        })
        .unwrap();
        assert!(!disabled.openapi_enabled);
    }
}
