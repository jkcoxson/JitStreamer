// jkcoxson

const fs = require('fs');
let dbFile = require('./database.json');

module.exports = class Database {

    constructor() { }

    get(key) {
        return dbFile[key];
    }

    save() {
        fs.writeFileSync('./database.json', JSON.stringify(dbFile, null, 2));
    }

    set(key, value) {
        dbFile[key] = value;
        this.save();
    }


}