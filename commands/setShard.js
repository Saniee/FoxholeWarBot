const { SlashCommandBuilder, CommandInteraction } = require('discord.js');
const { Model } = require('mongoose');
/**
 * @type {Model}
 */
const serverConfig = require('../serverConfig.js')
module.exports = {
    data: new SlashCommandBuilder()
        .setName('set-server')
        .setDescription('Sets the server (Able, Baker) the bot will get data from. Guild Side!')
        .addStringOption(option => option.setName('server').setDescription('Choose which server to get data from.').setRequired(true).addChoices(
            { name: 'Able', value: 'shard1' },
            { name: 'Baker', value: 'shard2' }
        )),
    /**
     * 
     * @param {CommandInteraction} interaction 
     */
    async execute(interaction) {
        shardName = interaction.options.getString('server')

        await interaction.deferReply({ ephemeral: true });

        serverConfig.findOne({ guildId: interaction.guildId }, async (err, mongoDB) => {
            if (mongoDB) {
                if (shardName == 'shard1') {
                    mongoDB.shard = 'https://war-service-live.foxholeservices.com';
                    mongoDB.shardName = 'Shard_1';
                    mongoDB.save().catch((err) => console.log(err));
                    await interaction.editReply('Saved!')
                } else if (shardName == 'shard2') {
                    mongoDB.shard = 'https://war-service-live-2.foxholeservices.com';
                    mongoDB.shardName = 'Shard_1';
                    mongoDB.save().catch((err) => console.log(err));
                    await interaction.editReply('Saved!')
                }
            } else if (!mongoDB) {
                const newServerConfig = new serverConfig({
                    guildID: interaction.guildId,
                    shard: 'https://war-service-live.foxholeservices.com',
                    shardName: 'Shard_1',
                });

                await newServerConfig.save().catch((err) => console.log(err));
                await interaction.editReply('Saved!')
            } else if (err) {
                await interaction.editReply('An error occured!')
            }
        })
        console.log(`Saved option ${shardName} for ${interaction.guild.name}!`)
    }
}