use serenity::{
    builder::CreateApplicationCommandOption, model::prelude::command::CommandOptionType,
};

pub fn register(
    option: &mut CreateApplicationCommandOption,
) -> &mut CreateApplicationCommandOption {
    option
        .name("premade")
        .description("a pre-populated database with example data")
        .kind(CommandOptionType::String)
        .add_string_choice(
            "Ecommerce database with people, products, as well as buy and review relations(mini)",
            "surreal_deal_mini",
        )
        .add_string_choice(
            "Ecommerce database with people, products, as well as buy and review relations(large)",
            "surreal_deal",
        )
}
