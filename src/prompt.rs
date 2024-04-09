use std::io::{stdin, Write as _};

#[derive(Default)]
pub struct Prompt {
    buffer: String,
}
impl Prompt {
    pub fn read_line(&mut self, prompt: &str) -> anyhow::Result<&str> {
        print!("\n{prompt} ");
        let _ = std::io::stdout().flush();

        self.buffer.clear();
        stdin().read_line(&mut self.buffer)?;
        Ok(self.buffer.trim())
    }
}
