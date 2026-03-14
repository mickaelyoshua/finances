use crossterm::event::KeyCode;
use finances::ui::components::input::InputField;

// -- Basic insertion --

#[test]
fn insert_ascii_chars() {
    let mut field = InputField::new("Test");
    field.handle_key(KeyCode::Char('a'));
    field.handle_key(KeyCode::Char('b'));
    field.handle_key(KeyCode::Char('c'));
    assert_eq!(field.value, "abc");
    assert_eq!(field.cursor, 3);
}

#[test]
fn insert_multibyte_utf8() {
    let mut field = InputField::new("Test");
    field.handle_key(KeyCode::Char('é'));
    field.handle_key(KeyCode::Char('ã'));
    assert_eq!(field.value, "éã");
    assert_eq!(field.cursor, 2); // 2 chars, not bytes
}

// -- Backspace --

#[test]
fn backspace_removes_char_before_cursor() {
    let mut field = InputField::new("Test").with_value("abc");
    field.handle_key(KeyCode::Backspace);
    assert_eq!(field.value, "ab");
    assert_eq!(field.cursor, 2);
}

#[test]
fn backspace_at_start_does_nothing() {
    let mut field = InputField::new("Test").with_value("abc");
    field.cursor = 0;
    field.handle_key(KeyCode::Backspace);
    assert_eq!(field.value, "abc");
    assert_eq!(field.cursor, 0);
}

#[test]
fn backspace_multibyte_char() {
    let mut field = InputField::new("Test").with_value("aé");
    field.handle_key(KeyCode::Backspace);
    assert_eq!(field.value, "a");
    assert_eq!(field.cursor, 1);
}

// -- Delete --

#[test]
fn delete_removes_char_at_cursor() {
    let mut field = InputField::new("Test").with_value("abc");
    field.cursor = 1;
    field.handle_key(KeyCode::Delete);
    assert_eq!(field.value, "ac");
    assert_eq!(field.cursor, 1);
}

#[test]
fn delete_at_end_does_nothing() {
    let mut field = InputField::new("Test").with_value("abc");
    // cursor at end (3)
    field.handle_key(KeyCode::Delete);
    assert_eq!(field.value, "abc");
}

// -- Cursor movement --

#[test]
fn left_moves_cursor() {
    let mut field = InputField::new("Test").with_value("abc");
    field.handle_key(KeyCode::Left);
    assert_eq!(field.cursor, 2);
}

#[test]
fn left_at_start_stays() {
    let mut field = InputField::new("Test").with_value("abc");
    field.cursor = 0;
    field.handle_key(KeyCode::Left);
    assert_eq!(field.cursor, 0);
}

#[test]
fn right_moves_cursor() {
    let mut field = InputField::new("Test").with_value("abc");
    field.cursor = 1;
    field.handle_key(KeyCode::Right);
    assert_eq!(field.cursor, 2);
}

#[test]
fn right_at_end_stays() {
    let mut field = InputField::new("Test").with_value("abc");
    field.handle_key(KeyCode::Right);
    assert_eq!(field.cursor, 3); // stays at end
}

#[test]
fn home_moves_to_start() {
    let mut field = InputField::new("Test").with_value("abc");
    field.handle_key(KeyCode::Home);
    assert_eq!(field.cursor, 0);
}

#[test]
fn end_moves_to_end() {
    let mut field = InputField::new("Test").with_value("abc");
    field.cursor = 0;
    field.handle_key(KeyCode::End);
    assert_eq!(field.cursor, 3);
}

// -- Insert in the middle --

#[test]
fn insert_at_cursor_position() {
    let mut field = InputField::new("Test").with_value("ac");
    field.cursor = 1;
    field.handle_key(KeyCode::Char('b'));
    assert_eq!(field.value, "abc");
    assert_eq!(field.cursor, 2);
}

// -- with_value sets cursor to end --

#[test]
fn with_value_cursor_at_end() {
    let field = InputField::new("Test").with_value("hello");
    assert_eq!(field.cursor, 5);
}

#[test]
fn with_value_multibyte_cursor_counts_chars() {
    let field = InputField::new("Test").with_value("café");
    assert_eq!(field.cursor, 4); // 4 chars, not 5 bytes
}
