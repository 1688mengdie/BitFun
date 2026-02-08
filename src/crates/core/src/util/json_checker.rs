/// JSON integrity checker - detect whether streamed JSON is complete
///
/// Primarily used to check whether tool-parameter JSON in AI streaming responses has been fully received
#[derive(Debug)]
pub struct JsonChecker {
    buffer: String,
    stack: Vec<char>,
    in_string: bool,
}

impl JsonChecker {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            stack: Vec::new(),
            in_string: false,
        }
    }

    pub fn append(&mut self, s: &str) {
        self.buffer.push_str(s);

        let mut chars = s.chars().peekable();
        let mut escape_next = false;

        while let Some(ch) = chars.next() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if self.in_string => {
                    escape_next = true;
                }
                '"' => {
                    self.in_string = !self.in_string;
                }
                '{' if !self.in_string => {
                    self.stack.push('{');
                }
                '}' if !self.in_string => {
                    if !self.stack.is_empty() {
                        self.stack.pop();
                    }
                }
                _ => {}
            }
        }
    }

    pub fn get_buffer(&self) -> String {
        self.buffer.clone()
    }

    pub fn is_valid(&self) -> bool {
        self.stack.is_empty() && self.buffer.starts_with("{")
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.stack.clear();
        self.in_string = false;
    }
}

impl Default for JsonChecker {
    fn default() -> Self {
        Self::new()
    }
}
