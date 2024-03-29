import fs from "node:fs";
import path from "node:path";
import { Cron } from "croner";
import {
  Client,
  Collection,
  GatewayIntentBits,
  ActivityType,
  Events,
  PresenceUpdateStatus,
} from "discord.js";

import { token } from "../config.json";
import {
  GenerateMapChoices,
  PocketBaseLogin,
  createNewDefaultRecord,
  deleteRecord,
} from "./utils";

import PocketBase from "pocketbase";

const client = new Client({ intents: [GatewayIntentBits.Guilds] });

client.commands = new Collection();

var pb: PocketBase;

const foldersPath = path.join(__dirname, "commands");
const commandFolders = fs.readdirSync(foldersPath);

for (const folder of commandFolders) {
  const commandsPath = path.join(foldersPath, folder);
  const commandFiles = fs
    .readdirSync(commandsPath)
    .filter((file) => file.endsWith(".js") || file.endsWith(".ts"));
  for (const file of commandFiles) {
    const filePath = path.join(commandsPath, file);
    const command = require(filePath);
    if ("data" in command && "execute" in command) {
      client.commands.set(command.data.name, command);
    } else {
      console.log(
        `[WARNING] The command at ${filePath} is missing a required "data" or "execute" property.`
      );
    }
  }
}

client.once(Events.ClientReady, async () => {
  await GenerateMapChoices();
  var { pocketbase } = await PocketBaseLogin();
  pb = pocketbase;

  console.log(`Ready as ${client.user?.username}!`);
  console.log(`In ${client.guilds.cache.size} servers!`);

  client.user?.setStatus(PresenceUpdateStatus.Idle);
  client.user?.setActivity("Foxhole Wars", { type: ActivityType.Watching });

  Cron("0 0 * * *", async () => {
    if (client.user == null) return;
    client.user.setStatus(PresenceUpdateStatus.Idle);
    client.user.setActivity("Foxhole Wars", { type: ActivityType.Watching });
    await GenerateMapChoices();
  });
});

client.on(Events.InteractionCreate, async (interaction) => {
  if (interaction.isChatInputCommand()) {
    const command = client.commands.get(interaction.commandName);

    if (!command) return;

    try {
      await command.execute(interaction, pb);
    } catch (error) {
      console.error(error);
      await interaction.reply({
        content: "There was an error while executing this command!",
        ephemeral: true,
      });
    }
  } else if (interaction.isAutocomplete()) {
    const command = interaction.client.commands.get(interaction.commandName);

    if (!command) {
      console.error(
        `No command matching ${interaction.commandName} was found.`
      );
      return;
    }

    try {
      await command.autocomplete(interaction, pb);
    } catch (error) {
      console.error(error);
    }
  }
});

client.on(Events.GuildCreate, async (guild) => {
  await createNewDefaultRecord(pb, undefined, guild);
  console.log(`Joined server ${guild.name}, created default record!`);
  console.log(`Now in ${client.guilds.cache.size} servers.`);
});

client.on(Events.GuildDelete, async (guild) => {
  if (!guild.available) return;

  await deleteRecord(pb, guild);
  console.log(`Left server ${guild.name}.`);
  console.log(`Now In ${client.guilds.cache.size} servers.`);
});

client.login(token);
