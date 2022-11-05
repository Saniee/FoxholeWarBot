const fs = require('node:fs');
const path = require('node:path');
const { REST } = require('@discordjs/rest');
const { Routes } = require('discord.js');
const { clientId, guildId, token } = require('./config.json');
const prompt = require("prompt-sync")({ sigint: true });

const commands = [];
const commandsPath = path.join(__dirname, 'commands');
const commandFiles = fs.readdirSync(commandsPath).filter(file => file.endsWith('.js'));

for (const file of commandFiles) {
    const filePath = path.join(commandsPath, file);
    const command = require(filePath);
    commands.push(command.data.toJSON());
}

const rest = new REST({ version: '10' }).setToken(token);

var guild = prompt('Deploy to dev guild? :')

if (guild == 'y') {
    rest.put(Routes.applicationGuildCommands(clientId, guildId), { body: commands })
        .then(() => console.log('Successfully registered guild application commands.'))
        .catch(console.error);
} else {
    rest.put(Routes.applicationCommands(clientId), { body: commands })
        .then(() => console.log('Successfully registered global application commands.'))
        .catch(console.error);
}
