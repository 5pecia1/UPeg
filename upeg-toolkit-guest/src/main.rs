use std::io::{self, BufRead, Write};

use upeg_toolkit_guest::{Request, run_result};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => serde_json::to_string(&run_result(request))?,
            Err(error) => {
                serde_json::to_string(&upeg_core::ToolResult::Failure(upeg_core::ToolFailure {
                    error: upeg_core::ToolError {
                        code: "invalid_request".to_owned(),
                        message: error.to_string(),
                        details: None,
                    },
                }))?
            }
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
