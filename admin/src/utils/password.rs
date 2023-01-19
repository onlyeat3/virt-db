use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};

pub fn encode(plain_text:String, salt:String) -> anyhow::Result<String> {
    let digest = md5::compute(format!("{}{}",plain_text,salt));
    let encoded_password = format!("{:x}",digest);
    Ok(encoded_password)
}

#[test]
pub fn test_encode(){
    let salt: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    let encoded_password_result = encode("admin123".to_string(),salt.clone());
    println!("salt:{:?},password:{:?}",salt,encoded_password_result);
}