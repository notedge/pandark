const newElement = document.createElement('div')
newElement.className = 'dummy-class'

const dummyText = document.createTextNode('Dummy text, generated by sidebar.ts')
newElement.appendChild(dummyText)

document.body.appendChild(newElement)
