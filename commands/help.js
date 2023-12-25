const { SlashCommandBuilder, CommandInteraction, EmbedBuilder, Colors } = require('discord.js');

module.exports = {
    data: new SlashCommandBuilder()
        .setName('help')
        .setDescription('All commands!'),
    /**
     * 
     * @param {CommandInteraction} interaction 
     */
    async execute(interaction) {
        const help = new EmbedBuilder()
            .setColor(Colors.Aqua)
            .setDescription('Made by Saniee#0007')
            .setTitle('Commands:')
            .addFields(
                {
                    name: 'help',
                    value: 'This command.',
                },
                {
                    name: 'get-map',
                    value: 'Responds with an image of that Hex/Map chunk. With Colored icons and labels.',
                },
                {
                    name: 'maps',
                    value: 'Lists all active maps',
                },
                {
                    name: 'war-report',
                    value: 'Responds with information about a Hex/Map Chunk.',
                },
                {
                    name: 'war-state',
                    value: 'Gets the global state of the war. This command will default to not showing.',
                },
                {
                    name: 'set-server',
                    value: 'Sets the server (Able, Baker, Charlie) the bot will get data from. Guild Side!',
                }
            );

        await interaction.reply({ embeds: [help], ephemeral: true })
    }
}