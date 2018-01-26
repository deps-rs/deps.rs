use std::str::FromStr;

pub struct CrateName(String);

#[derive(Debug)]
pub struct CrateNameValidationError;

impl AsRef<str> for CrateName {
  fn as_ref(&self) -> &str {
    self.0.as_ref()
  }
}

impl FromStr for CrateName {
    type Err = CrateNameValidationError;

    fn from_str(input: &str) -> Result<CrateName, CrateNameValidationError> {
        let is_valid = input.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        });

        if !is_valid {
            Err(CrateNameValidationError)
        } else {
            Ok(CrateName(input.to_string()))
        }
    }
}
