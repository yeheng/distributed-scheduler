use cron::Schedule;
use std::str::FromStr;

fn main() {
    // Test different cron formats
    let expressions = vec![
        "0 0 * * *",      // 5 fields
        "0 0 0 * * *",    // 6 fields  
        "* * * * *",      // 5 fields
        "0 * * * * *",    // 6 fields
    ];
    
    for expr in expressions {
        match Schedule::from_str(expr) {
            Ok(_) => println!("✓ Valid: {}", expr),
            Err(e) => println!("✗ Invalid: {} - {}", expr, e),
        }
    }
}