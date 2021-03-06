import Discord, {
  Intents,
  Collection,
  TextChannel,
  GuildMember,
  MessageEmbed,
  TextBasedChannel,
} from 'discord.js';
import path from 'path';
import fs from 'fs';
import dotenv from 'dotenv';
import mongoose from 'mongoose';
import serverConfig from './mongodb/serverConfig';
require('console-stamp')(console);
dotenv.config();


const client = new Discord.Client({
  intents: [Intents.FLAGS.GUILDS, Intents.FLAGS.GUILD_MESSAGES],
});

export class CustomClient extends Discord.Client {
  static commands: Collection<unknown, any> = new Collection();
}

const commandFiles = fs
  .readdirSync(path.resolve(__dirname, 'commands'))
  .filter((file) => file.endsWith('.ts'));

for (const file of commandFiles) {
  const command = require(`./commands/${file}`);
  CustomClient.commands.set(command.name, command);
}

async function mongoConnect() {
  mongoose
    .connect(process.env.MONGODB_STRING!)
    .catch((err) => {
      console.log('Error Connecting to MongoDB');
      process.exit(1);
    })
    .then(async () => {
      console.log('Connected to MongoDB');
    });
}

client.on('ready', async () => {
  await mongoConnect();

  client.user!.setStatus('idle');
  client.user!.setActivity('Foxhole Wars', { type: 'WATCHING' });
  console.log('Bot is online!');
  console.log(`In ${client.guilds.cache.size} servers!`);
});

client.on('messageCreate', async (msg) => {
  let channelID = msg.channel.id;
  if (!msg.content.startsWith(process.env.PREFIX!) || msg.author.bot) {
    return;
  }

  const args = msg.content.slice(process.env.PREFIX!.length).trim().split(/ +/);

  const commandName = args.shift()!;

  const command: any =
    CustomClient.commands.get(commandName) ||
    CustomClient.commands.find(
      (command: any) =>
        command['aliases'] && command['aliases'].includes(commandName)
    );

  if (!command) {
    msg
      .reply({
        content: `\`${msg.content.slice(
          process.env.PREFIX!.length
        )}\` is not a command!`,
      })
      .catch((err) => {
        console.log(err);
        (channelID as unknown as TextBasedChannel)
          .send({
            content: `\`${msg.content.slice(
              process.env.PREFIX!.length
            )}\` is not a command!`,
          })
          .catch((err) => {
            console.log(err);
          });
      });
  } else {
    try {
      await command.execute(msg, args);
    } catch (err) {
      console.log(err);
    }
  }
});

client.on('guildCreate', async (guild) => {
  const newServerConfig = new serverConfig({
    guildID: guild.id,
    shard: 'https://war-service-live.foxholeservices.com',
    shardName: 'Shard_1',
  });

  newServerConfig.save().catch((err: any) => console.log(err));

  let found = false;

  guild.channels.cache.forEach((channel: any) => {
    if (!found) {
      if (channel.type == 'GUILD_TEXT') {
        if (
          channel
            .permissionsFor(client.user as unknown as GuildMember)
            .has('VIEW_CHANNEL') === true
        ) {
          if (
            channel
              .permissionsFor(client.user as unknown as GuildMember)
              .has('SEND_MESSAGES') == true
          ) {
            const inviteEmbed = new MessageEmbed()
              .setTitle(
                'FoxholeWarBot the discord utility for Foxhole the Game.'
              )
              .setDescription(
                'Thanks for inviting me! \n\n To see the commands type in `War!help`! \n To change your prefered Shard/Server type in `War!setShard {shard1 | shard2}`, The default is Shard 1/Live 1. \n\n Commands are `case sensitive`!'
              )
              .addFields(
                {
                  name: 'Source:',
                  value: 'https://github.com/Saniee/FoxholeWarBot',
                },
                {
                  name: 'Discord Support Server:',
                  value: 'https://discord.gg/9wzppSgXdQ',
                }
              )
              .setColor('DARK_AQUA');
            try {
              (channel as TextChannel).send({ embeds: [inviteEmbed] });
            } catch (err) {
              console.log(err);
              (guild.ownerId as unknown as GuildMember)
                .send(
                  'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
                )
                .catch((err: any) => {
                  console.log(err);
                });
            }

            console.log(`In ${client.guilds.cache.size} servers!`);

            found = true;
          }
        }
      }
    }
  });

  console.log(`Joined server ${guild.name}, created default table!`);
});

client.on('guildDelete', async (guild) => {
  serverConfig
    .deleteOne({ guildID: guild.id })
    .catch((err: any) => console.log(err));
  console.log(`Left server ${guild.name}, dropped db table!`);
  console.log(`In ${client.guilds.cache.size} servers!`);
});

client.login(process.env.TOKEN);
