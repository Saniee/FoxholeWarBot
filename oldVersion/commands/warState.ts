import axios from 'axios';
import { Message, MessageEmbed } from 'discord.js';
import request from 'request';
import { getConstantValue } from 'typescript';
import serverConfig from '../mongodb/serverConfig';

module.exports = {
  name: 'warState',
  description: 'Displays the war state.',
  async execute(msg: Message, args: string[]) {
    serverConfig.findOne(
      { guildID: msg.guildId },
      async (err: any, mongoDB: any) => {
        try {
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
                msg.reply({
                  content:
                    'Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2',
                });
                break;
              case 404:
                msg.reply({ content: 'No War State for that HEX found!' });
                break;
              case 200:
                console.log(
                  `Api request for: WarState, Response Code: ${
                    (await response).status
                  }`
                );

                const data: any = (await response).data;
                let startTime = new Date(data.conquestStartTime);
                const warState = new MessageEmbed()
                  .setColor('RED')
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

                try {
                  await msg.reply({ embeds: [warState] });
                } catch (err) {
                  console.log(err);
                }
            }
          } else if (!mongoDB) {
            msg.reply({
              content:
                'Shard setting missing, please run the command `War!setShard {shard1 | shard2}` to fix this issue!',
            });
          }
        } catch (err) {
          console.log(err);
          msg.author
            .send(
              'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
            )
            .catch((err) => {
              console.log(err);
            });
        }
      }
    );
  },
};
