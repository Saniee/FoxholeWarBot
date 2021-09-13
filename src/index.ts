import Discord, { Intents, Collection } from 'discord.js';
import path from 'path';
import fs from 'fs';
import dotenv from 'dotenv';

dotenv.config();

const client = new Discord.Client({
  intents: [Intents.FLAGS.GUILDS, Intents.FLAGS.GUILD_MESSAGES],
});

const commandFiles = fs
  .readdirSync(path.resolve(__dirname, 'commands'))
  .filter((file) => file.endsWith('.ts'));

client.commands = new Collection();

for (const file of commandFiles) {
  const command = require(`./commands/${file}`);
  client.commands.set(command.name, command);
}

client.on('ready', async () => {
  client.user!.setStatus('idle');
  client.user!.setActivity('Watching Shard 2', { type: 'WATCHING' });
  console.log('Bot is online!');
});

client.on('messageCreate', async (msg) => {
  if (!msg.content.startsWith(process.env.PREFIX!) || msg.author.bot) {
    return;
  }

  const args = msg.content.slice(process.env.PREFIX!.length).trim().split(/ +/);

  const commandName = args.shift()!;

  const command: any =
    client.commands.get(commandName) ||
    client.commands.find(
      (command: any) =>
        command['aliases'] && command['aliases'].includes(commandName)
    );

  if (!command) {
    msg.reply({
      content: `\`${msg.content.slice(
        process.env.PREFIX!.length
      )}\` is not a command!`,
    });
  }

  // console.log(commandName);

  try {
    await command.execute(msg, args);
  } catch (err) {
    console.log(err);
  }
});

client.login(process.env.TOKEN);
