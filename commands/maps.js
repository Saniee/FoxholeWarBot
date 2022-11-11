const { SlashCommandBuilder, CommandInteraction, EmbedBuilder, Colors } = require('discord.js');
const axios = require('axios')
const serverConfig = require('../serverConfig.js')

module.exports = {
    data: new SlashCommandBuilder()
        .setName('maps')
        .setDescription('Responds with all the Hexes available! Defaults to not showing the response.')
        .addBooleanOption(option => option.setName('show').setDescription('Show the response?')),
    /**
     * 
     * @param {CommandInteraction} interaction 
     */
    async execute(interaction) {
        show = !interaction.options.getBoolean('show')

        await interaction.deferReply({ ephemeral: show })

        serverConfig.findOne({ guildID: interaction.guildId }, async (err, mongoDB) => {
            if (mongoDB) {
                const response = axios.get(
                    `${mongoDB.shard}/api/worldconquest/maps`,
                    {
                        validateStatus: function (status) {
                            return status < 600;
                        },
                    }
                );

                const statusCode = (await response).status;

                switch (statusCode) {
                    case 503:
                        await interaction.editReply({
                            content:
                                'Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2',
                        });
                        break;
                    case 404:
                        await interaction.editReply({ content: 'Maps Not found!' });
                        break;
                    case 200:
                        console.log(
                            `Api request for: maps, Response Code: ${(await response).status
                            }`
                        );

                        const data = (await response).data;
                        let mapHexes = '';

                        for (var i = 0; i < data.length; i++) {
                            mapHexes = mapHexes + ` ${data[i]}\n`;
                        }

                        const maps = new EmbedBuilder()
                            .setColor(Colors.Aqua)
                            .setFooter({ text: 'Requested at' })
                            .setTimestamp(new Date())
                            .setTitle('All Hexes:')
                            .setDescription(`${mapHexes}`);
                        try {
                            await interaction.editReply({ embeds: [maps] });
                        } catch (err) {
                            console.log(err);
                            await interaction.editReply({ content: 'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ' })
                        }
                        break;
                }
            } else if (!mongoDB) {
                try {
                    await interaction.editReply({
                        content: 'Server setting missing, please run the command `/set-server` to fix this issue!',
                    });
                } catch (err) {
                    console.log(err);
                    await interaction.editReply({
                        content: 'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
                    })
                }
            }
        })
    }
}