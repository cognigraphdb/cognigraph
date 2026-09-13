use super::*;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

// Exercise the actual HTTP boundary: effort must accompany Luna without
// changing schema enforcement or imposing OpenAI options on custom models.
#[tokio::test]
async fn completion_requests_preserve_schema_and_scope_low_effort_to_luna() {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        for model in [DEFAULT_OPENAI_COMPLETION_MODEL, "custom-model"] {
            for strict in [true, false] {
                let schema = json!({
                    "type": "object",
                    "properties": { "answer": { "type": "string" } },
                    "required": ["answer"],
                    "additionalProperties": !strict
                });
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
                let capture = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    reader.read_line(&mut line).await.unwrap();
                    assert_eq!(line, "POST /v1/chat/completions HTTP/1.1\r\n");
                    let mut content_length = None;
                    let mut authorization = None;
                    loop {
                        line.clear();
                        assert!(reader.read_line(&mut line).await.unwrap() > 0);
                        if line == "\r\n" {
                            break;
                        }
                        let (name, value) = line.split_once(':').unwrap();
                        if name.eq_ignore_ascii_case("content-length") {
                            content_length = Some(value.trim().parse::<usize>().unwrap());
                        } else if name.eq_ignore_ascii_case("authorization") {
                            authorization = Some(value.trim().to_owned());
                        }
                    }
                    assert_eq!(authorization.as_deref(), Some("Bearer synthetic-test-key"));
                    let mut body = vec![0; content_length.unwrap()];
                    reader.read_exact(&mut body).await.unwrap();
                    let response = json!({
                        "choices": [{ "message": { "content": "{\"answer\":\"ok\"}" } }]
                    })
                    .to_string();
                    reader
                        .get_mut()
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                response.len(), response
                            )
                            .as_bytes(),
                        )
                        .await
                        .unwrap();
                    serde_json::from_slice::<Value>(&body).unwrap()
                });
                let provider = OpenAiCompletion::new(
                    "synthetic-test-key".into(),
                    Some(base_url),
                    Some(model.into()),
                )
                .unwrap();
                let result = provider.complete_json("system", "user", &schema).await.unwrap();
                assert_eq!(result, json!({ "answer": "ok" }));
                let request = capture.await.unwrap();
                assert_eq!(request["model"], model);
                assert_eq!(request["messages"], json!([
                    { "role": "system", "content": "system" },
                    { "role": "user", "content": "user" }
                ]));
                assert_eq!(request["response_format"], json!({
                    "type": "json_schema",
                    "json_schema": { "name": "response", "schema": schema, "strict": strict }
                }));
                if model == DEFAULT_OPENAI_COMPLETION_MODEL {
                    assert_eq!(request["reasoning_effort"], "low");
                } else {
                    assert!(request.get("reasoning_effort").is_none());
                }
            }
        }
    })
    .await
    .expect("completion wire test timed out");
}
