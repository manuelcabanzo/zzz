mod terminal {
    use std::process::{Command, Stdio};
    use std::io::{self, Write};

    pub struct Terminal {
        output: String,
        cwd: String,
    }

    impl Terminal {
        pub fn new() -> Self {
            Self {
                output: String::new(),
                cwd: std::env::current_dir().unwrap().display().to_string(),
            }
        }

        pub fn run_command(&mut self, command: &str) {
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&self.cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let mut output = String::new();
            io::copy(&mut io::BufReader::new(stdout), &mut output).unwrap();
            io::copy(&mut io::BufReader::new(stderr), &mut output).unwrap();

            self.output.push_str(&output);
        }

        pub fn get_output(&self) -> &str {
            &self.output
        }

        pub fn set_cwd(&mut self, cwd: &str) {
            self.cwd = cwd.to_string();
        }

        pub fn get_cwd(&self) -> &str {
            &self.cwd
        }
    }
}
