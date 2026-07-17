const menuButton = document.querySelector('.menu-button');
const navLinks = document.querySelector('.nav-links');

if (menuButton && navLinks) {
  menuButton.addEventListener('click', () => {
    const open = navLinks.classList.toggle('is-open');
    menuButton.setAttribute('aria-expanded', open);
  });

  navLinks.addEventListener('click', (event) => {
    if (event.target.closest('a')) {
      navLinks.classList.remove('is-open');
      menuButton.setAttribute('aria-expanded', 'false');
    }
  });
}

document.querySelectorAll('.copy-button').forEach((button) => {
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      button.textContent = 'Copied';
      setTimeout(() => { button.textContent = 'Copy'; }, 1500);
    } catch {
      button.textContent = 'Copy failed';
    }
  });
});
