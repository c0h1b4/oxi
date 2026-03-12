# Backend Issues

## Add Fuzzy Search to "Recipient" When Drafting an E-mail

When drafting an email, the "Recipient" field currently does not support fuzzy search, which can make it difficult to find the correct contact, especially if there are many contacts or if the user is unsure of the exact name or email address. Implementing a fuzzy search algorithm in the "Recipient" field would allow users to find contacts more easily by matching partial names or email addresses, improving the overall user experience when composing emails. The fuzzy search algorithm must lookup both contacts and "known" email addresses, which are email addresses that the user has previously sent (or received) emails to (or from) but may not be saved as contacts. This will ensure that users can easily find and select the correct recipient when drafting an email, even if they do not remember the exact name or email address.

# Frontend Issues

## Tag creation text box overflows and cuts off when sidebar isn't large enough

When the sidebar is too small, the tag creation text box overflows and cuts off. This can be fixed by either making the sidebar larger or implementing a responsive design that adjusts the size of the text box based on the available space. The tag creation text box does seem to be responsive, but it may not be properly adjusting to the smaller sidebar size. This issue can be resolved by ensuring that the text box is properly contained within the sidebar and does not overflow when the sidebar is resized.

## Email content is unreadable in dark mode due to font color

When viewing email content in dark mode, the font color is set to dark, making it unreadable against the dark background. To fix this issue, the font color should be changed to a lighter color that contrasts well with the dark background, such as white or light gray. This will ensure that the email content is easily readable in dark mode.

## Improper/Small Render Size on Emails with Image/Html Content

When viewing emails that contain image or HTML content, the render size is often too small, making it difficult to read or view the content properly. This issue can be resolved by implementing a responsive design that allows the email content to adjust its size based on the available space. Additionally, ensuring that images and HTML elements are properly scaled and do not exceed the maximum width of the email container can help improve the readability and overall user experience when viewing such emails.

## Improper HTML rendering in email content

When viewing emails that contain HTML content, the rendering is often improper, leading to a poor user experience. This issue can be resolved by implementing a more robust HTML rendering engine that can properly interpret and display the various HTML elements and styles used in emails. Additionally, ensuring that the rendering engine is compatible with a wide range of HTML standards and practices can help improve the overall rendering quality and consistency across different emails. I suspect that the current rendering engine may not be fully compliant with modern HTML standards, or, possibly, that it is either sanitizing or stripping out certain HTML elements or styles that are commonly used in emails, leading to the improper rendering of email content. Another possibility is that the HTML css styles are conflicting with the application's own styles, causing the email content to be displayed incorrectly. To address this issue, it may be necessary to implement a more robust HTML rendering engine that can properly handle the various HTML elements and styles used in emails, while also ensuring that the application's own styles do not interfere with the rendering of email content. Check how roundcube's email client handles HTML rendering, as it is known for its robust handling of HTML emails, and see if there are any techniques or libraries that can be used to improve the rendering quality in this application.

## When the "Enable desktop notifications" popup shows up in the top of the screen, vertical scrolling of the entire application is enabled, which is not ideal

When the "Enable desktop notifications" popup appears at the top of the screen, it causes vertical scrolling of the entire application, which can be disruptive to the user experience. This issue can be resolved by ensuring that the popup is properly positioned and does not affect the layout of the rest of the application. One possible solution is to use a fixed position for the popup, allowing it to appear above the content without causing any scrolling. Additionally, implementing a more subtle and non-intrusive design for the popup can help minimize its impact on the overall user experience.

## Flashing issue when switching dark mode on and off

When switching between dark mode and light mode, there is a flashing issue that occurs on some specific elements, mainly unread emails and the "Mail" button/icon on the sidebar

## Animations

No animations have been implemented yet, but they must be added in the future to enhance the user experience. Animations can be used for various interactions, such as opening and closing emails, transitioning between different views, or providing visual feedback when performing actions like sending an email or deleting a message. Implementing smooth and subtle animations can make the application feel more responsive and engaging for users. Animations should be opt-outable for users who prefer a more minimalistic experience or have accessibility needs, ensuring that the application remains inclusive and user-friendly for all users, but defaulting to the beautiful animations for users who enjoy a more dynamic interface.

# General Issues (Backend + Frontend Related)
