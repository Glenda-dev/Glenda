#include <stdio.h>
#include <glenda/console.h>

int main(int argc, char *argv[])
{
    glenda_printf("Hello from C world (libglenda)!\n");
    printf("Hello from C world (musl)!\n");

    char buf[64];
    printf("Please enter your name: ");
    if (fgets(buf, sizeof(buf), stdin))
    {
        printf("Hello, %s!\n", buf);
    }

    return 0;
}
