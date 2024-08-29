use validator::validate_email;
#[derive(Debug)]
pub struct SubscriberEmail(String);
    impl SubscriberEmail {
        pub fn parse(s: String) -> Result<SubscriberEmail, String> {
            if validate_email(&s) {
                Ok(Self(s))
            } else {
                    Err(format!("{} is not a valid subscriber email.", s))
            }
        }
    }

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests{
    use fake::faker::internet::en::SafeEmail;
    use fake::Fake; 
    use super::SubscriberEmail;
    use claim::{assert_err, assert_ok};
    use quickcheck::{Arbitrary, Gen};
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use rand::RngCore;

    #[test]
    fn empty_string_is_rejected(){
        let email= "".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }
    #[test]
    fn email_missing_at_symbol_is_rejected(){
        let email= "kkgmail.com".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }
    #[test]
    fn email_missing_subject_is_rejected(){
        let email= "@gmail.com".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }

    // clone and debug are required by quickcheck
    #[derive(Debug, Clone)]
    struct ValidEmailFixture(pub String);

    impl Arbitrary for ValidEmailFixture {
        fn arbitrary(g: &mut Gen) -> Self {
            let seed = 42; // For reproducibility
            let mut rng = StdRng::seed_from_u64(seed);

            // Generate a random email address using SafeEmail and the custom RNG
            let email = SafeEmail().fake_with_rng(&mut rng);
            Self(email)
        }
    }
 
    #[quickcheck_macros::quickcheck] //generates multiple test cases based on the ValidEmailFixture type.
    fn valid_emails_are_parsed_successfully(valid_email: ValidEmailFixture)->bool {
       SubscriberEmail::parse(valid_email.0).is_ok()    
    }
}