use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use teloxide::{
    dispatching::UpdateFilterExt, prelude::*, types::UserId, utils::command::BotCommands,
};

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
    openai: Client<OpenAIConfig>,
    histories: Arc<Mutex<HashMap<UserId, Vec<ChatCompletionRequestMessage>>>>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();
    pretty_env_logger::init();

    let bot = Bot::from_env();
    log::info!("Bot started");

    let config = OpenAIConfig::new()
        .with_api_base(std::env::var("OPENAI_API_BASE").unwrap())
        .with_api_key(std::env::var("OPENAI_API_KEY").unwrap());

    let context = Arc::new(Context {
        openai: Client::with_config(config),
        histories: Arc::new(Mutex::new(HashMap::new())),
    });

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

    let model = "qwen-plus";
    let prompt = msg.text().unwrap();

    let user_history = context
        .histories
        .lock()
        .unwrap()
        .entry(user_id)
        .or_default()
        .clone();

    let mut request_messages = user_history;
    request_messages.push(
        ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()
            .unwrap()
            .into(),
    );

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(request_messages)
        .build()
        .unwrap();

    let mut stream = context.openai.chat().create_stream(request).await.unwrap();

    let mut tokens = 0;
    let mut response = String::new();
    while let Some(Ok(res)) = stream.next().await {
        if let Some(content) = res.choices[0].delta.content.as_deref() {
            response += content;
        }

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

    {
        let mut histories = context.histories.lock().unwrap();
        let user_messages = histories.entry(user_id).or_default();
        user_messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()
                .unwrap()
                .into(),
        );
        user_messages.push(
            ChatCompletionRequestAssistantMessageArgs::default()
                .content(response.clone())
                .build()
                .unwrap()
                .into(),
        );

        // Keep only the last 10 messages
        while user_messages.len() > 10 {
            user_messages.remove(0);
        }
    }

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

    let user_id = msg.from.as_ref().unwrap().id;

    match cmd {
        Command::Start => {
            bot.send_message(msg.chat.id, "Hello!").await?;
        }
        Command::Clear => {
            {
                let mut histories = context.histories.lock().unwrap();
                if let Some(user_messages) = histories.get_mut(&user_id) {
                    user_messages.clear();
                }
            }
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
