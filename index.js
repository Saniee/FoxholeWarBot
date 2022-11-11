const fs = require('node:fs');
const path = require('node:path');
const { Cron } = require('croner');
const mongoose = require('mongoose')
const serverConfig = require('./serverConfig.js');
const { Client, Collection, GatewayIntentBits, CommandInteraction, ActivityType, Status, Presence, PresenceUpdateStatus } = require('discord.js');
const { token, MONGODB_STRING } = require('./config.json');

const client = new Client({ intents: [GatewayIntentBits.Guilds] });

client.commands = new Collection();
const commandsPath = path.join(__dirname, 'commands');
const commandFiles = fs.readdirSync(commandsPath).filter(file => file.endsWith('.js'));

for (const file of commandFiles) {
    const filePath = path.join(commandsPath, file);
    const command = require(filePath);
    client.commands.set(command.data.name, command);
}

async function mongoConnect() {
    mongoose.connect(MONGODB_STRING).catch((err) => {
        console.log(err)
        console.log('Error Connecting to MongoDB');
        process.exit(1);
    }).then(async () => {
        console.log('Connected to MongoDB');
    });
}

client.once('ready', async () => {
    await mongoConnect();

    console.log(`Ready as ${client.user.username}!`);
    console.log(`In ${client.guilds.cache.size} servers!`);

    client.user.setStatus(PresenceUpdateStatus.Idle);
    client.user.setActivity('Foxhole Wars', { type: ActivityType.Watching });

    Cron('0 0 * * *', () => {
        client.user.setStatus(PresenceUpdateStatus.Idle);
        client.user.setActivity('Foxhole Wars', { type: ActivityType.Watching });
    })
});

client.on('interactionCreate',
    /**
     * 
     * @param {CommandInteraction} interaction 
     * @returns 
     */
    async interaction => {
        if (!interaction.isChatInputCommand()) return;

        const command = client.commands.get(interaction.commandName);

        if (!command) return;

        try {
            await command.execute(interaction);
        } catch (error) {
            console.error(error);
            await interaction.reply({ content: 'There was an error while executing this command!', ephemeral: true });
        }
    }
)

client.on('guildCreate', async (guild) => {
    const newServerConfig = new serverConfig({
        guildID: guild.id,
        shard: 'https://war-service-live.foxholeservices.com',
        shardName: 'Able',
    });
    newServerConfig.save().catch((err) => console.log(err));
    console.log(`Joined server ${guild.name}, created default table!`);
});

client.on('guildDelete', async (guild) => {
    serverConfig.deleteOne({ guildID: guild.id }).catch((err) => console.log(err));
    console.log(`Left server ${guild.name}, dropped db table!`);
    console.log(`In ${client.guilds.cache.size} servers!`);
});

client.login(token);