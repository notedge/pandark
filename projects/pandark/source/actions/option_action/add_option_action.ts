import { browser } from 'webextension-polyfill-ts'

// import {addUl} from '../../lib/functions'

async function get_url() {
    const tabs= await browser.tabs.query({currentWindow: true, active: true});
    console.log(tabs[0]);

    console.log(window.location.href)
    // await copyPageUrl()


}

async function copyPageUrl() {
    try {
        await navigator.clipboard.writeText(location.href);
        console.log('Page URL copied to clipboard');
    } catch (err) {
        console.error('Failed to copy: ', err);
    }
}


const addOptionBtn = document.querySelector('#convert') as HTMLButtonElement
console.log(addOptionBtn);

addOptionBtn.addEventListener('click', get_url)
