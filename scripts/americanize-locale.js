#!/usr/bin/env node
const fs = require('fs');

// Read the en-GB.json file
const gbContent = fs.readFileSync(process.argv[2], 'utf8');

// British to American spelling conversions
const conversions = {
  // -ise to -ize
  'organise': 'organize',
  'Organise': 'Organize',
  'organising': 'organizing',
  'Organising': 'Organizing',
  'organised': 'organized',
  'Organised': 'Organized',
  'customise': 'customize',
  'Customise': 'Customize',
  'customising': 'customizing',
  'customised': 'customized',
  'synchronise': 'synchronize',
  'Synchronise': 'Synchronize',
  'synchronising': 'synchronizing',
  'synchronised': 'synchronized',
  'authorise': 'authorize',
  'Authorise': 'Authorize',
  'authorising': 'authorizing',
  'authorised': 'authorized',
  'categorise': 'categorize',
  'categorised': 'categorized',
  
  // -our to -or  
  'colour': 'color',
  'Colour': 'Color',
  'favour': 'favor',
  'Favour': 'Favor',
  'behaviour': 'behavior',
  'Behaviour': 'Behavior',
  'honour': 'honor',
  'Honour': 'Honor',
  
  // -re to -er
  'centre': 'center',
  'Centre': 'Center',
  
  // -ogue to -og
  'catalogue': 'catalog',
  'Catalogue': 'Catalog',
  'dialogue': 'dialog',
  'Dialogue': 'Dialog',
  
  // -yse to -yze
  'analyse': 'analyze',
  'Analyse': 'Analyze',
  'analysing': 'analyzing',
  'analysed': 'analyzed'
};

let usContent = gbContent;

// Apply all conversions
for (const [british, american] of Object.entries(conversions)) {
  usContent = usContent.replace(new RegExp(british, 'g'), american);
}

// Write the result
fs.writeFileSync(process.argv[3], usContent);
console.log('Americanized en-US.json created');
