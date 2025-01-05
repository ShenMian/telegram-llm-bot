use futures::stream::StreamExt;
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use teloxide::prelude::*;
use teloxide::{dispatching::UpdateFilterExt, utils::command::BotCommands};

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display this message")]
    Help,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init();

    let bot = Bot::from_env();
    log::info!("Bot started");

    Dispatcher::builder(
        bot,
        dptree::entry()
            .branch(
                Update::filter_message()
                    .filter_command::<Command>()
                    .endpoint(handle_command),
            )
            .branch(Update::filter_message().endpoint(handle_message)),
    )
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
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

async fn handle_command(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    log::info!("{} called command {:?}", msg.chat.username().unwrap(), cmd);

    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
    };

    Ok(())
}
