// Selector de lista en terminal (flechas + enter), sin dependencias.
import { emitKeypressEvents } from 'node:readline';
import { bold, cyan, dim } from './output.mjs';

const plain = (s) => String(s).replace(/\x1b\[[0-9;]*m/g, '');
const pad = (s, n) => s + ' '.repeat(Math.max(0, n - plain(s).length));

export function canPrompt() {
  return Boolean(process.stdin.isTTY && process.stdout.isTTY);
}

/**
 * Muestra `items` y devuelve el `value` elegido, o null si se cancela.
 * @param {{title:string, items:{label:string, value:any}[], startIndex?:number}} options
 */
export function select({ title, items, startIndex = 0 }) {
  if (!canPrompt()) throw new Error('se necesita una terminal interactiva');

  return new Promise((resolve) => {
    let index = Math.min(Math.max(startIndex, 0), items.length - 1);
    let painted = 0;

    const paint = () => {
      if (painted) process.stdout.write(`\x1b[${painted}A\x1b[J`);
      const width = Math.max(...items.map((item) => plain(item.label).length));
      const lines = ['', `  ${bold(title)}`, ''];
      items.forEach((item, i) => {
        const pointer = i === index ? cyan('›') : ' ';
        const text = pad(item.label, width);
        lines.push(`  ${pointer} ${i === index ? text : dim(text)}`);
      });
      lines.push('', `  ${dim('↑↓ mover · enter elegir · esc cancelar')}`);
      process.stdout.write(`${lines.join('\n')}\n`);
      painted = lines.length;
    };

    const finish = (value) => {
      process.stdin.removeListener('keypress', onKey);
      process.stdin.setRawMode(false);
      process.stdin.pause();
      if (painted) process.stdout.write(`\x1b[${painted}A\x1b[J`);
      resolve(value);
    };

    const onKey = (str, key) => {
      if (key.ctrl && key.name === 'c') return finish(null);
      if (key.name === 'escape') return finish(null);
      if (key.name === 'return' || key.name === 'enter') return finish(items[index].value);
      if (key.name === 'up' || (key.name === 'k' && !key.ctrl)) {
        index = (index - 1 + items.length) % items.length;
        return paint();
      }
      if (key.name === 'down' || (key.name === 'j' && !key.ctrl)) {
        index = (index + 1) % items.length;
        return paint();
      }
      if (/^[1-9]$/.test(str)) {
        const n = Number(str) - 1;
        if (n < items.length) finish(items[n].value);
      }
      return undefined;
    };

    emitKeypressEvents(process.stdin);
    process.stdin.setRawMode(true);
    process.stdin.resume();
    paint();
    process.stdin.on('keypress', onKey);
  });
}
