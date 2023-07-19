extern crate henchman;
extern crate rpassword;

use henchman::password;

fn main() {
    let plaintext = rpassword::prompt_password("Password: ").unwrap();
    let hashed = password::hash_password(&plaintext);
    println!("{}", hashed);
}