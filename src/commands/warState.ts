import { Message, MessageEmbed } from 'discord.js';
import request from 'request';
import serverConfig from '../mongodb/serverConfig';

module.exports = {
  name: 'warState',
  description: 'Displays the war state.',
  async execute(msg: Message, args: string[]) {
    serverConfig.findOne({ guildID: msg.guildId }, (err: any, mongoDB: any) => {
      try {
        if (mongoDB) {
          const warStateURL = `${mongoDB.shard}/api/worldconquest/war`;

          request(warStateURL, async function (error, response, body) {
            console.log(
              `Api request for: WarState, Response Code: ${response.statusCode}`
            );

            const data = JSON.parse(body);
            let startTime = new Date(data.conquestStartTime);
            const warState = new MessageEmbed()
              .setColor('RED')
              .setFields(
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
          });
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
    });
  },
};
