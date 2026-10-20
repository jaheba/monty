const source = [1, , 3];
const visits = [];
const mapped = source.map(function(value, index, array) {
  visits.push([value, index, array === source, this.offset]);
  if (index === 0) array.push(9);
  return value + this.offset;
}, {offset: 10});
console.log(JSON.stringify([mapped, visits, source.length, 1 in mapped]));
console.log(Array.prototype.map.name, Array.prototype.map.length, Array.prototype.map.toString());
try { new Array.prototype.map(); } catch (error) { console.log(error.name); }
console.log([1, 2].map(function(prefix, value) { return prefix + value; }.bind(null, 10)).join(','));
try { [1, 2].map(value => { if (value === 2) throw new Error('stop'); return value; }); } catch (error) { console.log(error.message); }
