const { SlashCommandBuilder, CommandInteraction, EmbedBuilder, Colors } = require('discord.js');
const axios = require('axios')
const fs = require('node:fs');
const path = require('node:path');
const serverConfig = require('../serverConfig.js')

module.exports = {
    data: new SlashCommandBuilder()
        .setName('war-report')
        .setDescription('Responds with information about a Hex/Map Chunk.')
        .addStringOption(option => option.setName('map-name').setDescription('Name of the Hex you want displayed.').setRequired(true).setAutocomplete(true)),
    /**
     * 
     * @param {CommandInteraction} interaction 
     */
    async autocomplete(interaction) {
        serverConfig.findOne({ guildID: interaction.guildId }, async (err, MongoDB) => {
            if (MongoDB === null) {
                const focusedValue = interaction.options.getFocused();
                const choices = [
                    "There was an error. First try to run /set-server.",
                    "If that doesn't work go to the support server."
                ]
                const filtered = choices.filter(choice => choice.startsWith(focusedValue));

                let options;
                options = filtered

                await interaction.respond(
                    options.map(choice => ({ name: choice.replace("Hex", ""), value: choice })),
                );
            } else if (MongoDB.shardName === null) {
                const focusedValue = interaction.options.getFocused();
                const choices = [
                    "There was an error. First try to run /set-server.",
                    "If that doesn't work go to the support server."
                ]
                const filtered = choices.filter(choice => choice.startsWith(focusedValue));

                let options;
                options = filtered

                await interaction.respond(
                    options.map(choice => ({ name: choice.replace("Hex", ""), value: choice })),
                );
            } else {
                
                const fileData = fs.readFileSync(
                    path.resolve(
                        __dirname,
                        `../cache/mapChoices/${MongoDB.shardName}Hexes.json`
                    ),
                    { encoding: 'utf-8', flag: 'r' }
                );
    
                mapData = await JSON.parse(fileData);
                const focusedValue = interaction.options.getFocused();
                const choices = mapData
                const filtered = choices.filter(choice => choice.startsWith(focusedValue));
                
                let options;
                if (filtered.length > 25) {
                    options = filtered.slice(0, 25)
                } else {
                    options = filtered
                }
                
                await interaction.respond(
                    options.map(choice => ({ name: choice.replace("Hex", ""), value: choice })),
                );
            }
        })
    },
    /**
     * 
     * @param {CommandInteraction} interaction 
     */
    async execute(interaction) {
        mapName = interaction.options.getString('map-name')

        interaction.deferReply({ ephemeral: true })

        serverConfig.findOne({ guildID: interaction.guildId }, (err, mongoDB) => {
            if (mongoDB) {
                fs.readFile(
                    path.resolve(
                        __dirname,
                        `../cache/warReport/${mapName}${mongoDB.shardName}.json`
                    ),
                    'utf-8',
                    async (err, fileData) => {
                        if (fileData) {
                            const cache = JSON.parse(fileData);
                            await apiRequest(cache, mongoDB, interaction, mapName);
                        } else if (!fileData) {
                            const cache = {
                                version: '0',
                            };
                            await apiRequest(cache, mongoDB, interaction, mapName);
                        } else {
                            console.log(err);
                            interaction.editReply('An error has occured!');
                        }
                    }
                );
            } else if (!mongoDB) {
                interaction.editReply({
                    content: 'Shard setting missing, please run the command `War!setShard {shard1 | shard2}` to fix this issue!'
                });
            }
        });
    }
}

async function apiRequest(cache, mongoDB, interaction, mapName) {
    const warReportURL = `${mongoDB.shard}/api/worldconquest/warReport/${mapName}`;

    try {
        const response = axios.get(warReportURL, {
            validateStatus: function (status) {
                return status < 600;
            },
            headers: { 'If-None-Match': `"${cache.version}"` },
        });

        const statusCode = (await response).status;

        switch (statusCode) {
            case 503:
                interaction.editReply({ content: 'Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2' });
                break;
            case 404:
                interaction.editReply({ content: 'No War Report for that Hex found!' });
                break;
            case 200:
                console.log(
                    `Api request for: WarReport, Response Code: ${statusCode}, File: Not Up-to Date!`
                );

                const data = (await response).data;

                serverConfig.findOne(
                    { guildID: interaction.guildId },
                    (err, mongoDB) => {
                        fs.writeFile(
                            path.resolve(
                                __dirname,
                                `../cache/warReport/${mapName}${mongoDB.shardName}.json`
                            ),
                            JSON.stringify(data, null, 2),
                            'utf-8',
                            (err) => {
                                if (err) {
                                    console.log(err);
                                } else {
                                    console.log(`File: Updated!`);
                                }
                            }
                        );
                    }
                );

                const warReport = new EmbedBuilder()
                    .addFields(
                        {
                            name: 'Total Enlistments',
                            value: `${data.totalEnlistments}`,
                        },
                        {
                            name: 'Colonial Casualties',
                            value: `${data.colonialCasualties}`,
                        },
                        {
                            name: 'Warden Casualties',
                            value: `${data.wardenCasualties}`,
                        },
                        {
                            name: 'Day Of War',
                            value: `${data.dayOfWar}`,
                        }
                    )
                    .setFooter({ text: 'Requested at' })
                    .setTimestamp(new Date());
                try {
                    await interaction.editReply({ embeds: [warReport] });
                } catch (err) {
                    console.log(err);
                    await interaction.editReply({ content: 'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ' })
                }
                break;
            case 304:
                console.log(
                    `Api Request for: WarReport, Response Code: ${statusCode}, File: Up-to Date!`
                );

                const warReportCached = new EmbedBuilder()
                    .addFields(
                        {
                            name: 'Total Enlistments',
                            value: `${cache.totalEnlistments}`,
                        },
                        {
                            name: 'Colonial Casualties',
                            value: `${cache.colonialCasualties}`,
                        },
                        {
                            name: 'Warden Casualties',
                            value: `${cache.wardenCasualties}`,
                        },
                        {
                            name: 'Day Of War',
                            value: `${cache.dayOfWar}`,
                        }
                    )
                    .setFooter({ text: 'Requested at' })
                    .setTimestamp(new Date());
                try {
                    await interaction.editReply({ embeds: [warReportCached] });
                } catch (err) {
                    console.log(err);
                    await interaction.editReply({ content: 'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ' })
                }
                break;
        }
    } catch (err) {
        console.log(err);
        await interaction.editReply({ content: 'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ' })
    }
}