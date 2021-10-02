import { Message, MessageEmbed } from 'discord.js';
import fs from 'fs';
import path from 'path';
import request from 'request';
import serverConfig from '../mongodb/serverConfig';

module.exports = {
  name: 'warReport',
  description: 'Displays War Report for the Map/Hex',
  async execute(msg: Message, args: string[]) {
    if (args[0]) {
      serverConfig.findOne({ guildID: msg.guildId }, (err: any, data: any) => {
        if (data) {
          fs.readFile(
            path.resolve(
              __dirname,
              `../cache/warReport/${args[0]}${data.shardName}.json`
            ),
            'utf-8',
            async (err, fileData) => {
              if (fileData) {
                const cache = JSON.parse(fileData);
                await apiRequest(cache, data);
              } else if (!fileData) {
                const cache = {
                  version: '0',
                };
                await apiRequest(cache, data);
              } else {
                console.log(err);
                msg.reply('An error has occured!');
              }
            }
          );
        } else if (!data) {
          msg.reply({
            content:
              'Shard setting missing, please run the command `War!setShard {shard1 | shard2}` to fix this issue!',
          });
        }
      });
    } else {
      msg.reply({ content: 'Please specify the Map/Hex for the War Report!' });
    }

    async function apiRequest(cache: any, mongoDB: any) {
      const warReportURL = {
        url: `${mongoDB.shard}/api/worldconquest/warReport/${args[0]}`,
        headers: { 'If-None-Match': `"${cache.version}"` },
      };

      try {
        request(warReportURL, async function (error, response, body) {
          switch (response.statusCode) {
            case 404:
              msg.reply({ content: 'War Report for that map wasnt found!' });
              break;
            case 200:
              console.log(
                `Api request for: WarReport, Response Code: ${response.statusCode}, File: Not Up-to Date!`
              );

              const data = JSON.parse(body);

              serverConfig.findOne(
                { guildID: msg.guildId },
                (err: any, mongoDB: any) => {
                  fs.writeFile(
                    path.resolve(
                      __dirname,
                      `../cache/warReport/${args[0]}${mongoDB.shardName}.json`
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

              const warReport = new MessageEmbed()
                .setFields(
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
                .setFooter('Requested at')
                .setTimestamp(new Date());
              try {
                await msg.reply({ embeds: [warReport] });
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
              break;
            case 304:
              console.log(
                `Api Request for: WarReport, Response Code: ${response.statusCode}, File: Up-to Date!`
              );

              const warReportCached = new MessageEmbed()
                .setFields(
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
                .setFooter('Requested at')
                .setTimestamp(new Date());
              try {
                await msg.reply({ embeds: [warReportCached] });
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
              break;
          }
        });
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
  },
};
