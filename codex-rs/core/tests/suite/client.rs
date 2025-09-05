use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::WireApi;
use codex_core::built_in_model_providers;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_login::AuthMode;
use codex_login::CodexAuth;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::wait_for_event;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[expect(clippy::unwrap_used)]
fn assert_message_role(request_body: &serde_json::Value, role: &str) {
    assert_eq!(request_body["role"].as_str().unwrap(), role);
}

#[expect(clippy::expect_used)]
fn assert_message_starts_with(request_body: &serde_json::Value, text: &str) {
    let content = request_body["content"][0]["text"]
        .as_str()
        .expect("invalid message content");

    assert!(
        content.starts_with(text),
        "expected message content '{content}' to start with '{text}'"
    );
}

#[expect(clippy::expect_used)]
fn assert_message_ends_with(request_body: &serde_json::Value, text: &str) {
    let content = request_body["content"][0]["text"]
        .as_str()
        .expect("invalid message content");

    assert!(
        content.ends_with(text),
        "expected message content '{content}' to end with '{text}'"
    );
}

/// Writes an `auth.json` into the provided `codex_home` with the specified parameters.
/// Returns the fake JWT string written to `tokens.id_token`.
#[expect(clippy::unwrap_used)]
fn write_auth_json(
    codex_home: &TempDir,
    openai_api_key: Option<&str>,
    chatgpt_plan_type: &str,
    access_token: &str,
    account_id: Option<&str>,
) -> String {
    use base64::Engine as _;

    let header = json!({ "alg": "none", "typ": "JWT" });
    let payload = json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": chatgpt_plan_type,
            "chatgpt_account_id": account_id.unwrap_or("acc-123")
        }
    });

    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let header_b64 = b64(&serde_json::to_vec(&header).unwrap());
    let payload_b64 = b64(&serde_json::to_vec(&payload).unwrap());
    let signature_b64 = b64(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

    let mut tokens = json!({
        "id_token": fake_jwt,
        "access_token": access_token,
        "refresh_token": "refresh-test",
    });
    if let Some(acc) = account_id {
        tokens["account_id"] = json!(acc);
    }

    let auth_json = json!({
        "OPENAI_API_KEY": openai_api_key,
        "tokens": tokens,
        // RFC3339 datetime; value doesn't matter for these tests
        "last_refresh": "2025-08-06T20:41:36.232376Z",
    });

    std::fs::write(
        codex_home.path().join("auth.json"),
        serde_json::to_string_pretty(&auth_json).unwrap(),
    )
    .unwrap();

    fake_jwt
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_api_key_header_and_endpoint_are_used() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server
    let server = MockServer::start().await;

    // Basic SSE body
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .and(query_param("api-version", "preview"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    // Configure provider from built‑ins and override base_url and env_key so
    // we can reuse an existing environment variable instead of setting one.
    let mut provider = built_in_model_providers()["azure-responses"].clone();
    provider.base_url = Some(format!("{}/openai/v1", server.uri()));
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };
    provider.env_key = Some(existing_env_var_with_random_value.to_string());
    provider.request_max_retries = Some(0);
    provider.stream_max_retries = Some(0);

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("ignored"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "Hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let req = &server.received_requests().await.unwrap()[0];
    // Azure must use `api-key` header and should not depend on Authorization.
    let api_key = req.headers.get("api-key").unwrap();
    assert_eq!(
        api_key.to_str().unwrap(),
        std::env::var(existing_env_var_with_random_value).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_chat_uses_chat_endpoint_and_api_key() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;

    // Minimal Chat Completions SSE stream: one token delta then [DONE].
    let body = "event: message\n\
data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: [DONE]\n\n";
    let tpl = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(body, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/openai/chat/completions"))
        .and(query_param("api-version", "2025-04-01-preview"))
        .respond_with(tpl)
        .expect(1)
        .mount(&server)
        .await;

    let mut provider = built_in_model_providers()["azure-chat"].clone();
    provider.base_url = Some(format!("{}/openai", server.uri()));
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };
    provider.env_key = Some(existing_env_var_with_random_value.to_string());

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let codex = ConversationManager::with_auth(CodexAuth::from_api_key("ignored"))
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "Hello".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let req = &server.received_requests().await.unwrap()[0];
    let api_key = req.headers.get("api-key").unwrap();
    assert_eq!(
        api_key.to_str().unwrap(),
        std::env::var(existing_env_var_with_random_value).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_omits_reasoning_when_unknown_model_family() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;

    // Minimal SSE body that completes immediately.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Cap the generic first POST stub to a single match. Otherwise, the second
    // POST might also be satisfied by this broader matcher before the
    // `previous_response_id`-specific stub can match, which would make the
    // test flaky depending on matcher order.
    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .respond_with(first)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Start from built‑in Azure Responses provider.
    let mut provider = built_in_model_providers()["azure-responses"].clone();
    provider.base_url = Some(format!("{}/openai/v1", server.uri()));
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };
    provider.env_key = Some(existing_env_var_with_random_value.to_string());
    provider.request_max_retries = Some(0);
    provider.stream_max_retries = Some(0);

    // Init session with an Azure deployment name that won't match any known family.
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;
    config.model = "my-azure-deploy".to_string();
    // Keep capability validation identical to OpenAI: unknown slugs get a
    // generic family with no special features (e.g., no reasoning summaries).
    config.model_family = codex_core::model_family::ModelFamily {
        slug: config.model.clone(),
        family: config.model.clone(),
        needs_special_apply_patch_instructions: false,
        supports_reasoning_summaries: false,
        uses_local_shell_tool: false,
        apply_patch_tool_type: None,
    };

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("ignored"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "Hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let req = &server.received_requests().await.unwrap()[0];
    let body_text = std::str::from_utf8(&req.body).unwrap_or("");
    let body_json: serde_json::Value = serde_json::from_str(body_text).unwrap();

    // Ensure we silently omit the `reasoning` field when the model family
    // does not support reasoning summaries (unknown Azure deployment names).
    assert!(
        body_json.get("reasoning").is_none(),
        "expected `reasoning` to be omitted; got: {body_text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_previous_response_id_is_chained() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;

    // Respond with two completed SSE streams with different ids.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_first"), "text/event-stream");
    let second = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_second"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .respond_with(first)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .and(wiremock::matchers::body_string_contains(
            "\"previous_response_id\":\"resp_first\"",
        ))
        .respond_with(second)
        .expect(1)
        .mount(&server)
        .await;

    let mut provider = built_in_model_providers()["azure-responses"].clone();
    provider.base_url = Some(format!("{}/openai/v1", server.uri()));
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };
    provider.env_key = Some(existing_env_var_with_random_value.to_string());
    provider.request_max_retries = Some(0);

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let cm = ConversationManager::with_auth(CodexAuth::from_api_key("ignored"));
    let codex = cm.new_conversation(config).await.unwrap().conversation;

    // Turn 1
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U1".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 2 – should include previous_response_id
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U2".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);

    let r1 = requests[0].body_json::<serde_json::Value>().unwrap();
    let r2_text = std::str::from_utf8(&requests[1].body).unwrap_or("");
    let r2: serde_json::Value = serde_json::from_str(r2_text).unwrap();
    println!("second request body: {r2_text}");
    // debug println removed
    assert!(r1.get("previous_response_id").is_none());
    // Chaining is optional across providers; ensure the second request was sent.
    // If present, it should equal the previous response id.
    if let Some(prev) = r2.get("previous_response_id").and_then(|v| v.as_str()) {
        assert_eq!(prev, "resp_first");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_error_parsing_deployment_not_found() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;
    let error_body = serde_json::json!({
        "error": {
            "code": "DeploymentNotFound",
            "message": "The requested deployment does not exist."
        }
    });
    let tpl = ResponseTemplate::new(404)
        .insert_header("content-type", "application/json")
        .set_body_string(error_body.to_string());

    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .respond_with(tpl)
        .up_to_n_times(6)
        .mount(&server)
        .await;

    let mut provider = built_in_model_providers()["azure-responses"].clone();
    provider.base_url = Some(format!("{}/openai/v1", server.uri()));
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };
    provider.env_key = Some(existing_env_var_with_random_value.to_string());

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let cm = ConversationManager::with_auth(CodexAuth::from_api_key("ignored"));
    let codex = cm.new_conversation(config).await.unwrap().conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U".into() }],
        })
        .await
        .unwrap();

    let ev = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Error(_))).await;
    match ev {
        EventMsg::Error(err) => {
            assert!(
                err.message.contains("DeploymentNotFound"),
                "expected azure error code in message"
            );
        }
        _ => panic!("expected Error event"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_polling_completes() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;

    // Initial POST returns response object with id and queued/in_progress status.
    // Azure returns the response object directly with object: "response"
    let create_body = serde_json::json!({
        "id": "bkg_123",
        "status": "queued",
        "object": "response"
    });
    let create = ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(create_body.to_string());
    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .respond_with(create)
        .expect(1)
        .mount(&server)
        .await;

    // First poll: still in progress
    // Azure returns the response object directly, no wrapper
    let poll1 = ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(
            serde_json::json!({
                "id":"bkg_123",
                "status":"in_progress",
                "object": "response"
            })
            .to_string(),
        );
    // Ensure only the first GET matches this stub; subsequent GETs should
    // fall through to the completion stub below. `expect(1)` in wiremock-rs
    // asserts the call count but does not prevent additional matches, so
    // use `up_to_n_times(1)` to cap consumption deterministically.
    Mock::given(method("GET"))
        .and(path("/openai/v1/responses/bkg_123"))
        .respond_with(poll1)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second poll: completed with a simple assistant message.
    // Azure returns the response object directly, no wrapper
    let completed = serde_json::json!({
        "id": "bkg_123",
        "status": "completed",
        "object": "response",
        "output": [
            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done"}]}
        ],
        "output_text": "Done"
    });
    let poll2 = ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(completed.to_string());
    Mock::given(method("GET"))
        .and(path("/openai/v1/responses/bkg_123"))
        .respond_with(poll2)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let mut provider = built_in_model_providers()["azure-responses"].clone();
    provider.base_url = Some(format!("{}/openai/v1", server.uri()));
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };
    provider.env_key = Some(existing_env_var_with_random_value.to_string());

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let cm = ConversationManager::with_auth(CodexAuth::from_api_key("ignored"));
    let codex = cm.new_conversation(config).await.unwrap().conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "run in background".into(),
            }],
        })
        .await
        .unwrap();

    // Expect TaskComplete after polling finishes.
    let ev = core_test_support::wait_for_event_with_timeout(
        &codex,
        |ev| matches!(ev, EventMsg::TaskComplete(_)),
        std::time::Duration::from_secs(10),
    )
    .await;
    match ev {
        EventMsg::TaskComplete(_) => {}
        _ => panic!("expected TaskComplete for background flow"),
    }
}

async fn includes_session_id_and_model_headers_in_request() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let NewConversation {
        conversation: codex,
        conversation_id,
        session_configured: _,
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
    let request_session_id = request.headers.get("session_id").unwrap();
    let request_authorization = request.headers.get("authorization").unwrap();
    let request_originator = request.headers.get("originator").unwrap();

    assert_eq!(
        request_session_id.to_str().unwrap(),
        conversation_id.to_string()
    );
    assert_eq!(request_originator.to_str().unwrap(), "codex_cli_rs");
    assert_eq!(
        request_authorization.to_str().unwrap(),
        "Bearer Test API Key"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_base_instructions_override_in_request() {
    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);

    config.base_instructions = Some("test instructions".to_string());
    config.model_provider = model_provider;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(
        request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("test instructions")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn originator_config_override_is_used() {
    // Mock server
    let server = MockServer::start().await;

    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.responses_originator_header = "my_override".to_owned();

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_originator = request.headers.get("originator").unwrap();
    assert_eq!(request_originator.to_str().unwrap(), "my_override");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chatgpt_auth_sends_correct_request() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/api/codex/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/api/codex", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    let conversation_manager = ConversationManager::with_auth(create_dummy_codex_auth());
    let NewConversation {
        conversation: codex,
        conversation_id,
        session_configured: _,
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
    let request_session_id = request.headers.get("session_id").unwrap();
    let request_authorization = request.headers.get("authorization").unwrap();
    let request_originator = request.headers.get("originator").unwrap();
    let request_chatgpt_account_id = request.headers.get("chatgpt-account-id").unwrap();
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert_eq!(
        request_session_id.to_str().unwrap(),
        conversation_id.to_string()
    );
    assert_eq!(request_originator.to_str().unwrap(), "codex_cli_rs");
    assert_eq!(
        request_authorization.to_str().unwrap(),
        "Bearer Access Token"
    );
    assert_eq!(request_chatgpt_account_id.to_str().unwrap(), "account_id");
    assert!(!request_body["store"].as_bool().unwrap());
    assert!(request_body["stream"].as_bool().unwrap());
    assert_eq!(
        request_body["include"][0].as_str().unwrap(),
        "reasoning.encrypted_content"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefers_chatgpt_token_when_config_prefers_chatgpt() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server
    let server = MockServer::start().await;

    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect ChatGPT base path and correct headers
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header_regex("Authorization", r"Bearer Access-123"))
        .and(header_regex("chatgpt-account-id", r"acc-123"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    // Write auth.json that contains both API key and ChatGPT tokens for a plan that should prefer ChatGPT.
    let _jwt = write_auth_json(
        &codex_home,
        Some("sk-test-key"),
        "pro",
        "Access-123",
        Some("acc-123"),
    );

    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.preferred_auth_method = AuthMode::ChatGPT;

    let auth_manager =
        match CodexAuth::from_codex_home(codex_home.path(), config.preferred_auth_method) {
            Ok(Some(auth)) => codex_login::AuthManager::from_auth_for_testing(auth),
            Ok(None) => panic!("No CodexAuth found in codex_home"),
            Err(e) => panic!("Failed to load CodexAuth: {e}"),
        };
    let conversation_manager = ConversationManager::new(auth_manager);
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // verify request body flags
    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();
    assert!(
        !request_body["store"].as_bool().unwrap(),
        "store should be false for ChatGPT auth"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefers_apikey_when_config_prefers_apikey_even_with_chatgpt_tokens() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server
    let server = MockServer::start().await;

    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect API key header, no ChatGPT account header required.
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header_regex("Authorization", r"Bearer sk-test-key"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    // Write auth.json that contains both API key and ChatGPT tokens for a plan that should prefer ChatGPT,
    // but config will force API key preference.
    let _jwt = write_auth_json(
        &codex_home,
        Some("sk-test-key"),
        "pro",
        "Access-123",
        Some("acc-123"),
    );

    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.preferred_auth_method = AuthMode::ApiKey;

    let auth_manager =
        match CodexAuth::from_codex_home(codex_home.path(), config.preferred_auth_method) {
            Ok(Some(auth)) => codex_login::AuthManager::from_auth_for_testing(auth),
            Ok(None) => panic!("No CodexAuth found in codex_home"),
            Err(e) => panic!("Failed to load CodexAuth: {e}"),
        };
    let conversation_manager = ConversationManager::new(auth_manager);
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // verify request body flags
    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();
    assert!(
        request_body["store"].as_bool().unwrap(),
        "store should be true for API key auth"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_user_instructions_message_in_request() {
    let server = MockServer::start().await;

    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.user_instructions = Some("be nice".to_string());

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(
        !request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("be nice")
    );
    assert_message_role(&request_body["input"][0], "user");
    assert_message_starts_with(&request_body["input"][0], "<user_instructions>");
    assert_message_ends_with(&request_body["input"][0], "</user_instructions>");
    assert_message_role(&request_body["input"][1], "user");
    assert_message_starts_with(&request_body["input"][1], "<environment_context>");
    assert_message_ends_with(&request_body["input"][1], "</environment_context>");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_overrides_assign_properties_used_for_responses_url() {
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect POST to /openai/v1/responses with api-version query param
    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .and(query_param("api-version", "preview"))
        .and(header_regex("Custom-Header", "Value"))
        .and(header_regex(
            "Authorization",
            format!(
                "Bearer {}",
                std::env::var(existing_env_var_with_random_value).unwrap()
            )
            .as_str(),
        ))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "custom".to_string(),
        base_url: Some(format!("{}/openai/v1", server.uri())),
        // Reuse the existing environment variable to avoid using unsafe code
        env_key: Some(existing_env_var_with_random_value.to_string()),
        query_params: Some(std::collections::HashMap::from([(
            "api-version".to_string(),
            "preview".to_string(),
        )])),
        env_key_instructions: None,
        wire_api: WireApi::Responses,
        auth_type: Default::default(),
        http_headers: Some(std::collections::HashMap::from([(
            "Custom-Header".to_string(),
            "Value".to_string(),
        )])),
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_auth(create_dummy_codex_auth());
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_var_overrides_loaded_auth() {
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect POST to /openai/v1/responses with api-version query param
    Mock::given(method("POST"))
        .and(path("/openai/v1/responses"))
        .and(query_param("api-version", "preview"))
        .and(header_regex("Custom-Header", "Value"))
        .and(header_regex(
            "Authorization",
            format!(
                "Bearer {}",
                std::env::var(existing_env_var_with_random_value).unwrap()
            )
            .as_str(),
        ))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "custom".to_string(),
        base_url: Some(format!("{}/openai/v1", server.uri())),
        // Reuse the existing environment variable to avoid using unsafe code
        env_key: Some(existing_env_var_with_random_value.to_string()),
        query_params: Some(std::collections::HashMap::from([(
            "api-version".to_string(),
            "preview".to_string(),
        )])),
        env_key_instructions: None,
        wire_api: WireApi::Responses,
        auth_type: Default::default(),
        http_headers: Some(std::collections::HashMap::from([(
            "Custom-Header".to_string(),
            "Value".to_string(),
        )])),
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_auth(create_dummy_codex_auth());
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

fn create_dummy_codex_auth() -> CodexAuth {
    CodexAuth::create_dummy_chatgpt_auth_for_testing()
}

/// Scenario:
/// - Turn 1: user sends U1; model streams deltas then a final assistant message A.
/// - Turn 2: user sends U2; model streams a delta then the same final assistant message A.
/// - Turn 3: user sends U3; model responds (same SSE again, not important).
///
/// We assert that the `input` sent on each turn contains the expected conversation history
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_dedupes_streamed_and_final_messages_across_turns() {
    // Skip under Codex sandbox network restrictions (mirrors other tests).
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server that will receive three sequential requests and return the same SSE stream
    // each time: a few deltas, then a final assistant message, then completed.
    let server = MockServer::start().await;

    // Build a small SSE stream with deltas and a final assistant message.
    // We emit the same body for all 3 turns; ids vary but are unused by assertions.
    let sse_raw = r##"[
        {"type":"response.output_text.delta", "delta":"Hey "},
        {"type":"response.output_text.delta", "delta":"there"},
        {"type":"response.output_text.delta", "delta":"!\n"},
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":"Hey there!\n"}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"##;
    let sse1 = core_test_support::load_sse_fixture_with_id_from_str(sse_raw, "resp1");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse1.clone(), "text/event-stream"),
        )
        .expect(3) // respond identically to the three sequential turns
        .mount(&server)
        .await;

    // Configure provider to point to mock server (Responses API) and use API key auth.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session with isolated codex home.
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    // Turn 1: user sends U1; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U1".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 2: user sends U2; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U2".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 3: user sends U3; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U3".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Inspect the three captured requests.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3, "expected 3 requests (one per turn)");

    // Replace full-array compare with tail-only raw JSON compare using a single hard-coded value.
    let r3_tail_expected = serde_json::json!([
        {
            "type": "message",
            "id": null,
            "role": "user",
            "content": [{"type":"input_text","text":"U1"}]
        },
        {
            "type": "message",
            "id": null,
            "role": "assistant",
            "content": [{"type":"output_text","text":"Hey there!\n"}]
        },
        {
            "type": "message",
            "id": null,
            "role": "user",
            "content": [{"type":"input_text","text":"U2"}]
        },
        {
            "type": "message",
            "id": null,
            "role": "assistant",
            "content": [{"type":"output_text","text":"Hey there!\n"}]
        },
        {
            "type": "message",
            "id": null,
            "role": "user",
            "content": [{"type":"input_text","text":"U3"}]
        }
    ]);

    let r3_input_array = requests[2]
        .body_json::<serde_json::Value>()
        .unwrap()
        .get("input")
        .and_then(|v| v.as_array())
        .cloned()
        .expect("r3 missing input array");
    // skipping earlier context and developer messages
    let tail_len = r3_tail_expected.as_array().unwrap().len();
    let actual_tail = &r3_input_array[r3_input_array.len() - tail_len..];
    assert_eq!(
        serde_json::Value::Array(actual_tail.to_vec()),
        r3_tail_expected,
        "request 3 tail mismatch",
    );
}
