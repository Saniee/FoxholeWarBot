import { Canvas, createCanvas, loadImage } from 'canvas';
import axios from 'axios';
import serverConfig from '../mongodb/serverConfig';
import fs from 'fs';
import path from 'path';

export = {
  make: async function (canvas: Canvas, cache: any, mongoDB: any) {
    return canvas;
  },
};
