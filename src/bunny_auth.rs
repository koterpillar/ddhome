use std::env;

const BUNNY_API_KEY_ENV: &str = "BUNNY_API_KEY";

pub fn read_bunny_api_key() -> Result<String, String> {
    match env::var(BUNNY_API_KEY_ENV) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) => Err(format!(
            "environment variable {BUNNY_API_KEY_ENV} is set but empty"
        )),
        Err(_) => Err(format!(
            "environment variable {BUNNY_API_KEY_ENV} is not set"
        )),
    }
}
