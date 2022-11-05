const { SlashCommandBuilder, CommandInteraction, Colors, EmbedBuilder } = require('discord.js');
const { Model } = require('mongoose');
/**
 * @type {Model}
 */
const serverConfig = require('../serverConfig.js')
module.exports = {
    data: new SlashCommandBuilder()
        .setName('war-state')
        .setDescription('example desc.'),
    /**
     * 
     * @param {CommandInteraction} interaction 
     */
    async execute(interaction) {
        interaction.deferReply({ ephemeral: false })

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
                        interaction.editReply({
                            content:
                                'Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2',
                        });
                        break;
                    case 404:
                        interaction.editReply({ content: 'No War State for that HEX found!' });
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
                            .setFields(
                                { name: 'Shard/Server', value: `${mongoDB.shardName}` },
                                { name: 'War Number', value: `${data.warNumber}` },
                                { name: 'Winner', value: `${data.winner}` },
                                { name: 'Conquest Start Time', value: `${startTime}` },
                                {
                                    name: 'Required Victory Towns',
                                    value: `${data.requiredVictoryTowns}`,
                                }
                            )
                            .setFooter('Requested at')
                            .setTimestamp(new Date());

                        interaction.editReply({ embeds: [warState] })
                }
            }
        }
    }
}