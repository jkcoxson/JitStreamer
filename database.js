// jkcoxson

let dbFile = require('./database.json');

module.exports = class Database {

    constructor() { }

    get(key) {
        return dbFile[key];
    }

    set(key, value) {
        dbFile[key] = value;
        self.save();
    }

    save() {
        fs.writeFileSync('./database.json', JSON.stringify(dbFile, null, 2));
    }
}