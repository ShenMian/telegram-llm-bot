use futures::stream::StreamExt;
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use teloxide::prelude::*;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init();

    let bot = Bot::from_env();
    log::info!("Bot started");

    teloxide::repl(bot, handle_message).await;
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    let ollama = Ollama::default();

    let model = "qwen2.5:latest";
    let prompt = msg.text().unwrap();

    let mut stream = ollama
        .generate_stream(GenerationRequest::new(
            model.to_string(),
            prompt.to_string(),
        ))
        .await
        .unwrap();

    let message = bot.send_message(msg.chat.id, "...").await.unwrap();

    let mut buffer = String::new();
    while let Some(res) = stream.next().await {
        let responses = res.unwrap();
        for response in responses {
            buffer += &response.response;
            let suffix = if response.done { "" } else { "..." };
            bot.edit_message_text(message.chat.id, message.id, format!("{buffer} {suffix}"))
                .await
                .unwrap();
        }
    }

    log::info!("{}: {}", message.chat.username().unwrap(), prompt);
    log::info!("LLM: {}", buffer);

    Ok(())
}
