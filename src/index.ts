import fs from "node:fs";
import path from "node:path";
import { Cron } from "croner";
import mongoose from "mongoose";
import {
  Client,
  Collection,
  GatewayIntentBits,
  ActivityType,
  Events,
  PresenceUpdateStatus,
} from "discord.js";

import serverConfig from "./serverConfig";
import { token, CollectionName } from "../config.json";
import { GenerateMapChoices, PocketBaseLogin } from "./Util";

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
    // Set a new item in the Collection with the key as the command name and the value as the exported module
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
  const data = {
    guildId: guild.id,
    shard: "https://war-service-live.foxholeservices.com",
    shardName: "Able",
  };
  await pb.collection(CollectionName).create(data);
  console.log(`Joined server ${guild.name}, created default record!`);
  console.log(`Now in ${client.guilds.cache.size} servers.`);
});

client.on(Events.GuildDelete, async (guild) => {
  await pb
    .collection(CollectionName)
    .delete(
      (
        await pb
          .collection(CollectionName)
          .getFirstListItem(`guildId=${guild.id}`)
      ).id
    );
  console.log(`Left server ${guild.name}, dropped record!`);
  console.log(`Now In ${client.guilds.cache.size} servers.`);
});

client.login(token);
