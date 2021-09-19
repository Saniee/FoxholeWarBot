import { Canvas, createCanvas, loadImage } from 'canvas';
import { Message, MessageAttachment, MessageEmbed } from 'discord.js';
import request from 'request';
import path from 'path';
import fs from 'fs';
import serverConfig from '../mongodb/serverConfig';

module.exports = {
  name: 'getMap',
  descriptio: 'Generates a Hex Image of the map with icons.',
  async execute(msg: Message, args: string[]) {
    if (args[0]) {
      serverConfig.findOne({ guildID: msg.guildId }, (err: any, data: any) => {
        if (data) {
          fs.readFile(
            path.resolve(
              __dirname,
              `../cache/getMap/${args[0]}${data.shardName}.json`
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
      msg.reply({
        content:
          'Please specify the Map/Hex! You can get a list of Maps/Hex with the command !maps',
      });
    }

    async function apiRequest(cache: any, mongoDB: any) {
      const dynamicMapDataUrl = {
        url: `${mongoDB.shard}/api/worldconquest/maps/${args[0]}/dynamic/public`,
        headers: { 'If-None-Match': `"${cache.version}"` },
      };

      request(dynamicMapDataUrl, async function (error, response, body) {
        if (response.statusCode == 404) {
          msg.reply({
            content: 'There was nothing found for the Map/Hex Specified!',
          });
        } else if (response.statusCode == 200) {
          console.log(
            `Api request for: getMap, Response Code: ${response.statusCode}, File: Not Up-to Date!`
          );

          const data = JSON.parse(body);

          serverConfig.findOne(
            { guildID: msg.guildId },
            (err: any, mongoDB: any) => {
              fs.writeFile(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${mongoDB.shardName}.json`
                ),
                JSON.stringify(data, null, 2),
                'utf-8',
                (err) => {
                  if (err) {
                    console.log(err);
                  } else {
                    console.log('File: Updated!');
                  }
                }
              );
            }
          );

          const canvas = createCanvas(1024, 888);
          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(
              __dirname,
              `../assets/Images/MapsPNG/Map${args[0]}.png`
            )
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < data.mapItems.length; i++) {
            if (data.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
                )
              );
              // console.log(data.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (data.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   data.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}.png`
                )
              );
              // console.log(data.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }

          await send(canvas, new Date(data.lastUpdated));
        } else if (response.statusCode == 304) {
          console.log(
            `Api request for: getMap, Response Code: ${response.statusCode}, File: Up-to Date!`
          );

          const data = cache;

          const canvas = createCanvas(1024, 888);
          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(
              __dirname,
              `../assets/Images/MapsPNG/Map${args[0]}.png`
            )
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < data.mapItems.length; i++) {
            if (data.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
                )
              );
              // console.log(data.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (data.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   data.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}.png`
                )
              );
              // console.log(data.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }
          await send(canvas, new Date(cache.lastUpdated));
        }
      });
    }

    async function send(canvas: Canvas, lastUpdated: Date) {
      const attachment = new MessageAttachment(canvas.toBuffer(), 'image.png');

      const myEmbed = new MessageEmbed()
        .setDescription(`Last API Update: ` + `${lastUpdated}`)
        .setColor('GREEN')
        .setImage('attachment://image.png')
        .setFooter('Requested at')
        .setTimestamp(new Date());

      try {
        await msg.reply({ embeds: [myEmbed], files: [attachment] });
      } catch (err) {
        console.log(err);
      }
    }
  },
};
