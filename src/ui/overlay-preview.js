const replacements=['Homeguard','Witchful Thinking',"Upgrade Zhonya's"];
document.querySelectorAll('.reroll').forEach((button,index)=>button.addEventListener('click',()=>{button.closest('article').querySelector('h2').textContent=replacements[index];button.disabled=true;}));
document.querySelectorAll('.pick').forEach(button=>button.addEventListener('click',()=>{document.querySelector('#choices').hidden=true;document.querySelector('h1').textContent='Preview complete';document.querySelector('#complete').hidden=false;}));
