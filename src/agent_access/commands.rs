//! AppService commands are forwarded only in explicitly configured bot rooms.
use crate::shared::slash_commands::SlashCommand;
use matrix_sdk::{room::RoomMember, ruma::RoomId};

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "createbot",
        aliases: &[],
        description: "Create a managed bot",
        usage: "/createbot <username> <display name>",
    },
    SlashCommand {
        name: "deletebot",
        aliases: &[],
        description: "Delete a managed bot",
        usage: "/deletebot <Matrix ID>",
    },
    SlashCommand {
        name: "listbots",
        aliases: &[],
        description: "List managed bots",
        usage: "/listbots",
    },
    SlashCommand {
        name: "bothelp",
        aliases: &[],
        description: "Show BotFather help",
        usage: "/bothelp",
    },
    SlashCommand {
        name: "schedule",
        aliases: &[],
        description: "Schedule a bot task",
        usage: "/schedule <task>",
    },
    SlashCommand {
        name: "schedules",
        aliases: &[],
        description: "List scheduled tasks",
        usage: "/schedules",
    },
    SlashCommand {
        name: "unschedule",
        aliases: &[],
        description: "Remove a scheduled task",
        usage: "/unschedule <ID>",
    },
    SlashCommand {
        name: "allbots",
        aliases: &[],
        description: "Send to bots in this room",
        usage: "/allbots <message>",
    },
];
pub fn enabled(room: Option<&RoomId>, members: Option<&[RoomMember]>) -> bool {
    let settings = super::current();
    let bot = &settings.bot_settings;
    if !bot.enabled {
        return false;
    }
    let Some(room) = room else { return false };
    if bot.is_room_bound(room) {
        return true;
    }
    let Ok(botfather) = bot.resolved_bot_user_id(crate::sliding_sync::current_user_id().as_deref())
    else {
        return false;
    };
    members.is_some_and(|members| members.iter().any(|m| m.user_id() == botfather))
}
pub fn matching(enabled: bool, query: &str) -> impl Iterator<Item = &'static SlashCommand> {
    let query = query.to_ascii_lowercase();
    COMMANDS
        .iter()
        .filter(move |command| enabled && command.name.starts_with(&query))
}
pub fn contains(enabled: bool, name: &str) -> bool {
    enabled && (name.contains('@') || COMMANDS.iter().any(|c| c.name == name))
}

/// External agents can receive explicitly addressed commands even when the
/// separate AppService management controls are disabled.
pub fn addressed_to_registered(name: &str) -> bool {
    name.split_once('@').is_some_and(|(_, local)| {
        super::current()
            .agent_registry
            .agent_user_ids()
            .iter()
            .any(|id| id.localpart().eq_ignore_ascii_case(local))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn management_commands_require_explicit_context() {
        assert!(!super::contains(false, "listbots"));
        assert!(super::contains(true, "bothelp"));
        assert!(!super::contains(true, "leave"));
        assert_eq!(super::matching(true, "list").count(), 1);
        assert_eq!(super::matching(false, "").count(), 0);
    }
}
