import fs = require('fs');
import { join } from 'path';
this.root = join(fs.realpathSync('.'), 'x');
