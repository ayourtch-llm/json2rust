use std::process::Command;
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn test_simple_json_object() {
    let json_input = r#"{"name": "John", "age": 30, "active": true}"#;
    
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "{}", json_input).expect("Failed to write to temp file");
    
    let output = Command::new("cargo")
        .args(&["run", "--bin", "json2rust", "--", "-i", temp_file.path().to_str().unwrap(), "-n", "Person"])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    assert!(stdout.contains("pub struct Person"));
    assert!(stdout.contains("pub name: String"));
    assert!(stdout.contains("pub age: f64"));
    assert!(stdout.contains("pub active: bool"));
    assert!(stdout.contains("Serialize, Deserialize"));
}

#[test]
fn test_json_array() {
    let json_input = r#"[{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]"#;
    
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "{}", json_input).expect("Failed to write to temp file");
    
    let output = Command::new("cargo")
        .args(&["run", "--bin", "json2rust", "--", "-i", temp_file.path().to_str().unwrap(), "-n", "Users"])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    assert!(stdout.contains("pub struct Users"));
    assert!(stdout.contains("pub items: Vec<UsersItem>"));
    assert!(stdout.contains("pub struct UsersItem"));
    assert!(stdout.contains("pub id: f64"));
    assert!(stdout.contains("pub name: String"));
}

#[test]
fn test_nested_json() {
    let json_input = r#"{"user": {"name": "John", "profile": {"age": 30}}, "posts": [{"title": "Hello", "id": 1}]}"#;
    
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "{}", json_input).expect("Failed to write to temp file");
    
    let output = Command::new("cargo")
        .args(&["run", "--bin", "json2rust", "--", "-i", temp_file.path().to_str().unwrap(), "-n", "Root"])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    assert!(stdout.contains("pub struct Root"));
    assert!(stdout.contains("pub user: User"));
    assert!(stdout.contains("pub posts: Vec<PostsItem>"));
    assert!(stdout.contains("pub struct User"));
    assert!(stdout.contains("pub profile: Profile"));
    assert!(stdout.contains("pub struct Profile"));
    assert!(stdout.contains("pub struct PostsItem"));
}

#[test]
fn test_with_existing_struct() {
    let json_input = r#"{"name": "John", "age": 30, "email": "john@example.com"}"#;
    let existing_struct = r#"
        struct Person {
            name: String,
            age: i32,
        }
    "#;
    
    let mut json_file = NamedTempFile::new().expect("Failed to create temp file");
    write!(json_file, "{}", json_input).expect("Failed to write to temp file");
    
    let mut existing_file = NamedTempFile::new().expect("Failed to create temp file");
    write!(existing_file, "{}", existing_struct).expect("Failed to write to temp file");
    
    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "json2rust", "--", 
            "-i", json_file.path().to_str().unwrap(),
            "-e", existing_file.path().to_str().unwrap(),
            "-n", "Person"
        ])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    assert!(stdout.contains("pub struct Person"));
    assert!(stdout.contains("pub name: String"));
    assert!(stdout.contains("pub age: f64"));
    assert!(stdout.contains("pub email: String"));
}

#[test]
fn test_stdin_input() {
    let json_input = r#"{"message": "Hello World"}"#;
    
    let mut child = Command::new("cargo")
        .args(&["run", "--bin", "json2rust", "--", "-n", "Message"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn child process");
    
    let stdin = child.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(json_input.as_bytes()).expect("Failed to write to stdin");
    let _ = stdin;
    
    let output = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    assert!(stdout.contains("pub struct Message"));
    assert!(stdout.contains("pub message: String"));
}