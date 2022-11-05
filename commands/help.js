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
                    name: 'get-map {Map/Hex}',
                    value: 'Gets info about a Map/Hex',
                },
                {
                    name: 'maps',
                    value: 'Lists all active maps',
                },
                {
                    name: 'war-report {Map/Hex}',
                    value: 'Displays War Report for the Map/Hex',
                },
                {
                    name: 'war-state',
                    value: 'Displays the war state.',
                },
                {
                    name: 'set-server',
                    value: 'Sets Server (Able, Baker) to get data from.',
                }
            );

        await interaction.reply({ embeds: [help], ephemeral: true })
    }
}