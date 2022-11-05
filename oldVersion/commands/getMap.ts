import { Canvas, createCanvas, loadImage } from 'canvas';
import { Message, MessageAttachment, MessageEmbed } from 'discord.js';
import axios from 'axios';
import path from 'path';
import fs from 'fs';
import serverConfig from '../mongodb/serverConfig';

module.exports = {
  name: 'getMap',
  descriptio: 'Generates a Hex Image of the map with icons.',
  async execute(msg: Message, args: string[]) {
    if (args[0]) {
      serverConfig.findOne(
        { guildID: msg.guildId },
        async (err: any, MongoDB: any) => {
          if (MongoDB) {
            var dynamicCache;
            var staticCache;
            try {
              const fileData = fs.readFileSync(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${MongoDB.shardName}Dynamic.json`
                ),
                { encoding: 'utf-8', flag: 'r' }
              );
              dynamicCache = await JSON.parse(fileData);
            } catch (err) {
              dynamicCache = {
                version: '0',
              };
            }
            try {
              const fileData = fs.readFileSync(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${MongoDB.shardName}Static.json`
                ),
                { encoding: 'utf-8', flag: 'r' }
              );
              staticCache = await JSON.parse(fileData);
            } catch (err) {
              staticCache = {
                version: '0',
              };
            }

            await apiRequest(dynamicCache, staticCache, MongoDB);
          } else if (!MongoDB) {
            msg.reply({
              content:
                'Shard setting missing, please run the command `War!setShard {shard1 | shard2}` to fix this issue!',
            });
          }
        }
      );
    } else {
      msg
        .reply({
          content:
            'Please specify the Map/Hex! You can get a list of Maps/Hex with the command !maps',
        })
        .catch((err) => {
          console.log(err);
          msg.author
            .send(
              'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
            )
            .catch((err) => {
              console.log(err);
            });
        });
    }

    async function apiRequest(
      dynamicCache: any,
      staticCache: any,
      mongoDB: any
    ) {
      const dynamicMapDataUrl = `${mongoDB.shard}/api/worldconquest/maps/${args[0]}/dynamic/public`;
      const staticMapDataUrl = `${mongoDB.shard}/api/worldconquest/maps/${args[0]}/static`;

      // headers: { 'If-None-Match': `"${dynamicCache.version}"` }

      try {
        const canvas = createCanvas(1024, 888);

        const getDymanic = axios.get(dynamicMapDataUrl, {
          validateStatus: function (status) {
            return status < 600;
          },
          headers: { 'If-None-Match': `"${dynamicCache.version}"` },
        });

        const getStatic = axios.get(staticMapDataUrl, {
          validateStatus: function (status) {
            return status < 600;
          },
          headers: { 'If-None-Match': `"${staticCache.version}"` },
        });

        const dataDynamic: any = (await getDymanic).data;
        const statusCodeDynamic: any = (await getDymanic).status;

        const dataStatic: any = (await getStatic).data;
        const statusCodeStatic: any = (await getStatic).status;

        console.log(statusCodeDynamic + ' | ' + statusCodeStatic);
        
        if (statusCodeStatic == 503 || statusCodeDynamic == 503) {
          msg.reply({
            content:
              'Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2',
          });
        } else if (statusCodeDynamic == 404 || statusCodeStatic == 404) {
          msg.reply({
            content: 'There was nothing found for the Map/Hex Specified!',
          });
        } else if (statusCodeDynamic == 200 && statusCodeStatic == 200) {
          console.log(
            `Api request for: getMap, Response Codes: ${statusCodeDynamic}|${statusCodeStatic}, File: Not Up-to Date!`
          );

          serverConfig.findOne(
            { guildID: msg.guildId },
            (err: any, mongoDB: any) => {
              fs.writeFile(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${mongoDB.shardName}Dynamic.json`
                ),
                JSON.stringify(dataDynamic, null, 2),
                'utf-8',
                (err) => {
                  if (err) {
                    console.log(err);
                  } else {
                    console.log('Dynamic Data: Updated!');
                  }
                }
              );

              fs.writeFile(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${mongoDB.shardName}Static.json`
                ),
                JSON.stringify(dataStatic, null, 2),
                'utf-8',
                (err) => {
                  if (err) {
                    console.log(err);
                  } else {
                    console.log('Static Data: Updated!');
                  }
                }
              );
            }
          );

          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(
              __dirname,
              `../assets/Images/MapsPNG/Map${args[0]}.png`
            )
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < dataDynamic.mapItems.length; i++) {
            if (dataDynamic.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (dataDynamic.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }

          for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
            ctx.font = '20px serif';
            ctx.fillStyle = '#000000';
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.8,
              dataStatic.mapTextItems[i].y * 1000 * 0.935
            );
          }

          await send(canvas, new Date(dataDynamic.lastUpdated));
        } else if (statusCodeDynamic == 304 && statusCodeStatic == 304) {
          console.log(
            `Api request for: getMap, Response Code: ${statusCodeDynamic}, File: Up-to Date!`
          );

          const dataDynamic = dynamicCache;
          const dataStatic = staticCache;

          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(
              __dirname,
              `../assets/Images/MapsPNG/Map${args[0]}.png`
            )
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < dataDynamic.mapItems.length; i++) {
            if (dataDynamic.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (dataDynamic.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }

          for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
            ctx.font = '20px serif';
            ctx.fillStyle = '#000000';
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.8,
              dataStatic.mapTextItems[i].y * 1000 * 0.935
            );
          }

          await send(canvas, new Date(dynamicCache.lastUpdated));
        } else if (statusCodeDynamic != 200 && statusCodeStatic == 200) {
          console.log(
            `Api request for: getMap, Response Codes: ${statusCodeDynamic}|${statusCodeStatic}, File: Not Up-to Date!`
          );

          serverConfig.findOne(
            { guildID: msg.guildId },
            (err: any, mongoDB: any) => {
              fs.writeFile(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${mongoDB.shardName}Static.json`
                ),
                JSON.stringify(dataStatic, null, 2),
                'utf-8',
                (err) => {
                  if (err) {
                    console.log(err);
                  } else {
                    console.log('Static Data: Updated!');
                  }
                }
              );
            }
          );

          const dataDynamic = dynamicCache;

          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(
              __dirname,
              `../assets/Images/MapsPNG/Map${args[0]}.png`
            )
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < dataDynamic.mapItems.length; i++) {
            if (dataDynamic.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (dataDynamic.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }

          for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
            ctx.font = '20px serif';
            ctx.fillStyle = '#000000';
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.8,
              dataStatic.mapTextItems[i].y * 1000 * 0.935
            );
          }

          await send(canvas, new Date(dataDynamic.lastUpdated));
        } else if (statusCodeDynamic == 200 && statusCodeStatic != 200) {
          console.log(
            `Api request for: getMap, Response Codes: ${statusCodeDynamic}|${statusCodeStatic}, File: Not Up-to Date!`
          );

          serverConfig.findOne(
            { guildID: msg.guildId },
            (err: any, mongoDB: any) => {
              fs.writeFile(
                path.resolve(
                  __dirname,
                  `../cache/getMap/${args[0]}${mongoDB.shardName}Dynamic.json`
                ),
                JSON.stringify(dataDynamic, null, 2),
                'utf-8',
                (err) => {
                  if (err) {
                    console.log(err);
                  } else {
                    console.log('Dynamic Data: Updated!');
                  }
                }
              );
            }
          );

          const dataStatic = staticCache;

          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(
              __dirname,
              `../assets/Images/MapsPNG/Map${args[0]}.png`
            )
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < dataDynamic.mapItems.length; i++) {
            if (dataDynamic.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (dataDynamic.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
                )
              );
              // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                dataDynamic.mapItems[i].x * 1000 * 1.005,
                dataDynamic.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }

          for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
            ctx.font = '20px serif';
            ctx.fillStyle = '#000000';
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.8,
              dataStatic.mapTextItems[i].y * 1000 * 0.935
            );
          }

          await send(canvas, new Date(dataDynamic.lastUpdated));
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
