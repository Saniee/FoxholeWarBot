const { SlashCommandBuilder, CommandInteraction, Colors, EmbedBuilder } = require('discord.js');
const axios = require('axios');
const { Model } = require('mongoose');
/**
 * @type {Model}
 */
const serverConfig = require('../serverConfig.js')

module.exports = {
    data: new SlashCommandBuilder()
        .setName('war-state')
        .setDescription('Gets the global state of the war. This command will default to not showing.')
        .addBooleanOption(option => option.setName('show').setDescription('Show the response?')),
    /**
     * 
     * @param {CommandInteraction} interaction
     */
    async execute(interaction) {
        show = !interaction.options.getBoolean('show');

        await interaction.deferReply({ ephemeral: show })

        serverConfig.findOne({ guildId: interaction.guildId }, async (err, mongoDB) => {
            if (mongoDB) {
                const warStateURL = `${mongoDB.shard}/api/worldconquest/war`;

                const response = axios.get(warStateURL, {
                    validateStatus: function (status) {
                        return status < 600;
                    },
                });

                const statusCode = (await response).status;

                switch (statusCode) {
                    case 503:
                        await interaction.editReply({
                            content:
                                'Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2',
                        });
                        break;
                    case 404:
                        await interaction.editReply({ content: 'No War State for that HEX found!' });
                        break;
                    case 200:
                        console.log(
                            `Api request for: WarState, Response Code: ${(await response).status
                            }`
                        );
                        const data = (await response).data;
                        let startTime = new Date(data.conquestStartTime);
                        const warState = new EmbedBuilder()
                            .setColor(Colors.Red)
                            .addFields(
                                { name: 'Shard/Server', value: `${mongoDB.shardName}` },
                                { name: 'War Number', value: `${data.warNumber}` },
                                { name: 'Winner', value: `${data.winner}` },
                                { name: 'Conquest Start Time', value: `${startTime}` },
                                {
                                    name: 'Required Victory Towns',
                                    value: `${data.requiredVictoryTowns}`,
                                }
                            )
                            .setFooter({ text: 'Requested at:' })
                            .setTimestamp(new Date())

                        await interaction.editReply({ embeds: [warState] })
                        break;
                }
            }
        })
    }
}