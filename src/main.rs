use std::sync::{Arc, Mutex};

use futures::stream::StreamExt;
use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage},
    Ollama,
};
use teloxide::{dispatching::UpdateFilterExt, prelude::*, utils::command::BotCommands};

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Clear chat history")]
    Clear,
    #[command(description = "Display this message")]
    Help,
}

struct Context {
    ollama: Ollama,
    history: Arc<Mutex<Vec<ChatMessage>>>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();
    pretty_env_logger::init();

    let context = Arc::new(Context {
        ollama: Ollama::default(),
        history: Arc::new(Mutex::new(Vec::new())),
    });

    let bot = Bot::from_env();
    log::info!("Bot started");

    let schema = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(handle_command),
        )
        .branch(Update::filter_message().endpoint(handle_message));
    let mut dispatcher = Dispatcher::builder(bot, schema)
        .dependencies(dptree::deps![context])
        .enable_ctrlc_handler()
        .build();
    dispatcher.dispatch().await;
}

async fn handle_message(bot: Bot, msg: Message, context: Arc<Context>) -> ResponseResult<()> {
    let message = bot.send_message(msg.chat.id, "...").await.unwrap();

    let user_id = msg.from.as_ref().unwrap().id;

    let model = "qwen2.5:latest";
    let prompt = msg.text().unwrap();
    let mut stream = context
        .ollama
        .send_chat_messages_with_history_stream(
            context.history.clone(),
            ChatMessageRequest::new(
                model.to_string(),
                vec![ChatMessage::user(prompt.to_string())],
            ),
        )
        .await
        .unwrap();

    let mut tokens = 0;
    let mut response = String::new();
    while let Some(Ok(res)) = stream.next().await {
        response += res.message.content.as_str();

        tokens += 1;
        if tokens % 5 == 0 {
            bot.edit_message_text(message.chat.id, message.id, format!("{response} ..."))
                .await
                .unwrap();
        }
    }
    bot.edit_message_text(message.chat.id, message.id, &response)
        .await
        .unwrap();

    log::info!("{} ({user_id}): {prompt}", message.chat.username().unwrap());
    log::info!("LLM: {response}");

    Ok(())
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    context: Arc<Context>,
) -> ResponseResult<()> {
    log::info!("{} called command {:?}", msg.chat.username().unwrap(), cmd);

    match cmd {
        Command::Start => {
            bot.send_message(msg.chat.id, "Hello!").await?;
        }
        Command::Clear => {
            context.history.lock().unwrap().clear();
            bot.send_message(msg.chat.id, "Chat history cleared")
                .await?;
        }
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
    };

    Ok(())
}
